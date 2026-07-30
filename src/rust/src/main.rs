use std::env;
use std::path::Path;

mod config;
mod checkpoint;
mod log;
mod registry;
mod world;
mod world_sources;
mod world_command;
mod submit;
mod exec;
mod exec_file;
mod process;
mod gateway;
mod supervisor;
mod shell;
mod memory;
mod agent;
mod explore;
mod agent_loop;
mod agent_develop;

fn main() {
    let argv0 = env::args().next().unwrap_or_default();
    let name = Path::new(&argv0)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("boos");

    match name {
        "boos-submit"     => submit::main(),
        "boos-exec"       => exec::main(),
        "boos-process"    => process::main(),
        "boos-gateway"    => gateway::main(),
        "boos-supervisor" => supervisor::main(),
        "boos-shell"      => shell::main(),
        "boos-agent"      => agent::main(),
        _ => {
            eprintln!("Usage: boos-{{submit,exec,process,gateway,supervisor,shell,agent}} ...");
            std::process::exit(crate::config::EXIT_ERROR);
        }
    }
}
