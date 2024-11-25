pub mod command;
pub mod session;
pub mod stream_command;

#[cfg(test)]
pub(crate) mod tests;

pub const DEFAULT_SESSION_NAME: &str = "";

pub const SOCK_ENV_VAR_NAME: &str = "CHIMERA_SESSION_SOCK";

pub const COMMAND_LIMIT_BYTES: usize = 256;
