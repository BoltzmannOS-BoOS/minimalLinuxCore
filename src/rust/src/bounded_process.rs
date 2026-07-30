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
