use std::env;
use std::fs;
use std::io;
use std::path::Path;

use crate::config;

use super::{invalid_data, load_definitions, resolve_context, PrincipalContext};

const PRINCIPAL_ENV: &str = "BOOS_PRINCIPAL_ID";
const LEGACY_AGENT_ENV: &str = "BOOS_AGENT_ID";

pub fn current_context() -> io::Result<PrincipalContext> {
    let claimed_id = read_claimed_principal()?;
    let definitions = load_definitions()?;
    let status = fs::read_to_string("/proc/self/status")?;
    let effective_uid = parse_effective_uid(&status)?;
    resolve_context(
        &definitions,
        &claimed_id,
        effective_uid,
        Path::new(config::PRINCIPAL_RUNTIME_DIR),
    )
}

fn read_claimed_principal() -> io::Result<String> {
    match env::var(PRINCIPAL_ENV) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => match env::var(LEGACY_AGENT_ENV) {
            Ok(value) => Ok(value),
            Err(env::VarError::NotPresent) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "no BoOS principal identity is present",
            )),
            Err(env::VarError::NotUnicode(_)) => {
                Err(invalid_data("legacy agent identity is not valid Unicode"))
            }
        },
        Err(env::VarError::NotUnicode(_)) => {
            Err(invalid_data("principal identity is not valid Unicode"))
        }
    }
}

pub(super) fn parse_effective_uid(status: &str) -> io::Result<u32> {
    let value = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .and_then(|line| line.split_whitespace().nth(2))
        .ok_or_else(|| invalid_data("process status has no effective UID"))?;
    value
        .parse::<u32>()
        .map_err(|_| invalid_data("process effective UID is invalid"))
}

