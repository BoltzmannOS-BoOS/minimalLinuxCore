use std::env;
use std::path::Path;

mod config;
mod log;
mod registry;
mod submit;
mod exec;
mod process;
mod gateway;

fn main() {
    let argv0 = env::args().next().unwrap_or_default();
    let name = Path::new(&argv0)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("boos");

    match name {
        "boos-submit"  => submit::main(),
        "boos-exec"    => exec::main(),
        "boos-process" => process::main(),
        "boos-gateway" => gateway::main(),
        _ => {
            eprintln!("Usage: boos-{{submit,exec,process,gateway}} ...");
            std::process::exit(1);
        }
    }
}
