use std::{
    env,
    ffi::OsString,
    io::{self, Error},
};

use embedded_session::{
    command::{EmbeddedSessionCommand, SessionExecutable},
    stream_command::StreamCommand,
};
use tokio::{io::AsyncWriteExt, net::UnixSocket};

use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
/// Command line tool for managing chimera session
struct Args {
    #[argh(option, short = 's')]
    /// unix socket address
    socket_addr: Option<OsString>,

    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
/// Subcommands for managing the chimera session
enum Command {
    Terminate(TerminateCommand),
    Restart(RestartCommand),
}

#[derive(FromArgs, PartialEq, Debug)]
/// Terminate the chimera session
#[argh(subcommand, name = "terminate")]
struct TerminateCommand {}

#[derive(FromArgs, PartialEq, Debug)]
/// Restart the session
#[argh(subcommand, name = "restart")]
struct RestartCommand {
    #[argh(option, short = 'n')]
    /// name of the session will get launched upon restart
    name: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args: Args = argh::from_env();

    let sock_path = match args.socket_addr {
        Some(path) => path,
        None => match env::var_os(OsString::from(embedded_session::SOCK_ENV_VAR_NAME)) {
            Some(sock_addr) => sock_addr.as_os_str().to_os_string(),
            None => return Err(Error::new(io::ErrorKind::NotFound, format!("Could not find environment variable {}, are you running this tool from a session started with embedded-session-exec?", embedded_session::SOCK_ENV_VAR_NAME)))
        },
    };

    let socket = UnixSocket::new_stream()?;
    let mut stream = socket.connect(sock_path).await?;

    let encoder = StreamCommand::new(embedded_session::COMMAND_LIMIT_BYTES);

    let encoded_cmd = match args.command {
        Command::Terminate(_terminate_command) => encoder
            .encode(EmbeddedSessionCommand::Terminate)
            .map_err(|err| {
                std::io::Error::new(
                    io::ErrorKind::Other,
                    format!("Could not serialize the termination command: {}", err),
                )
            }),
        Command::Restart(restart_command) => encoder
            .encode(EmbeddedSessionCommand::Restart(SessionExecutable::new(
                restart_command.name.as_str(),
            )))
            .map_err(|err| {
                std::io::Error::new(
                    io::ErrorKind::Other,
                    format!("Could not serialize the restart command: {}", err),
                )
            }),
    }?;

    Ok(stream.write_all(encoded_cmd.as_slice()).await?)
}
