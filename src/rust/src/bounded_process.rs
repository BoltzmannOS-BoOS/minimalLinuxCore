use std::io::{self, Read};
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
}

struct CapturedStream {
    bytes: Vec<u8>,
    truncated: bool,
}

pub fn run_with_limits(
    command: &mut Command,
    max_stream_bytes: usize,
    timeout: Duration,
) -> io::Result<BoundedOutput> {
    // A separate process group lets a deadline terminate grandchildren that
    // inherited the output pipes. Killing only the direct child could leave
    // those pipes open and make the reader threads wait forever.
    command
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;

    let stdout = child.stdout.take().ok_or_else(|| {
        terminate_and_reap(&mut child);
        io::Error::other("child stdout pipe was not created")
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        terminate_and_reap(&mut child);
        io::Error::other("child stderr pipe was not created")
    })?;
    let stdout_reader = read_stream(stdout, max_stream_bytes);
    let stderr_reader = read_stream(stderr, max_stream_bytes);

    let deadline = Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now);
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                terminate_process_group(&mut child);
                break child.wait()?;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                terminate_and_reap(&mut child);
                let _ = join_stream(stdout_reader);
                let _ = join_stream(stderr_reader);
                return Err(error);
            }
        }
    };

    let stdout = join_stream(stdout_reader)?;
    let stderr = join_stream(stderr_reader)?;
    Ok(BoundedOutput {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        timed_out,
    })
}

fn read_stream<R: Read + Send + 'static>(
    mut stream: R,
    max_bytes: usize,
) -> thread::JoinHandle<io::Result<CapturedStream>> {
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
        let mut truncated = false;
        let mut buffer = [0u8; 8192];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let retained = (max_bytes - bytes.len()).min(count);
                    bytes.extend_from_slice(&buffer[..retained]);
                    truncated |= retained < count;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(CapturedStream { bytes, truncated })
    })
}

fn join_stream(
    reader: thread::JoinHandle<io::Result<CapturedStream>>,
) -> io::Result<CapturedStream> {
    reader
        .join()
        .map_err(|_| io::Error::other("child output reader panicked"))?
}

fn terminate_and_reap(child: &mut Child) {
    terminate_process_group(child);
    let _ = child.wait();
}

fn terminate_process_group(child: &mut Child) {
    const SIGKILL: i32 = 9;

    unsafe {
        // The child is still alive while its process-group ID is used, so the
        // negative PID cannot refer to a newly reused, unrelated group.
        let _ = kill(-(child.id() as i32), SIGKILL);
    }
    // Retain the standard-library kill as a fallback if group setup or kill
    // was rejected by the host.
    let _ = child.kill();
}

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn large_stdout_and_stderr_are_drained_but_not_retained_without_bound() {
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "i=0; while [ \"$i\" -lt 4096 ]; do \
             printf 123456789abcdef; printf fedcba987654321 >&2; \
             i=$((i + 1)); done",
        ]);

        let output =
            run_with_limits(&mut command, 1024, Duration::from_secs(5)).unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 1024);
        assert_eq!(output.stderr.len(), 1024);
        assert!(output.stdout_truncated);
        assert!(output.stderr_truncated);
        assert!(!output.timed_out);
    }

    #[test]
    fn process_group_is_terminated_when_the_deadline_expires() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 & wait"]);
        let started = Instant::now();

        let output =
            run_with_limits(&mut command, 1024, Duration::from_millis(100)).unwrap();

        assert!(output.timed_out);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
