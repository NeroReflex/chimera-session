use std::{collections::HashMap, env, ffi::OsString, path::Path, sync::Arc};

use tokio::{
    io::AsyncReadExt,
    net::{UnixListener, UnixStream},
    process::{Child, Command},
    sync::Mutex,
};

use tokio::select;

use crate::{command::*, stream_command::StreamCommand};

pub struct Session {
    listener: UnixListener,
    sock_addr: OsString,
    stream_interpreter: StreamCommand,
    command: SessionExecutable,
}

impl Session {
    pub fn new(socket_path: &Path) -> std::io::Result<Self> {
        let sock_addr = socket_path.as_os_str().to_os_string();
        let listener = UnixListener::bind(socket_path)?;
        let stream_interpreter = StreamCommand::new(crate::COMMAND_LIMIT_BYTES);
        let command = SessionExecutable::new(crate::DEFAULT_SESSION_NAME);
        Ok(Self {
            listener,
            sock_addr,
            stream_interpreter,
            command,
        })
    }

    fn start_child(&self) -> std::io::Result<Child> {
        let mut env_vars: HashMap<OsString, OsString> = env::vars_os().collect();
        env_vars.insert(
            OsString::from(crate::SOCK_ENV_VAR_NAME),
            self.sock_addr.clone(),
        );

        let mut command = Command::new(self.command.get_program());

        command
            .args(self.command.get_arguments())
            .kill_on_drop(true);

        for (key, value) in env_vars {
            command.env(key, value);
        }

        command.spawn()
    }

    pub async fn run(&mut self) -> std::io::Result<()> {
        let mut proc = self.start_child()?;

        let mut stream: Option<Arc<Mutex<UnixStream>>> = None;

        let mut exit_requested = false;

        while !exit_requested {
            match stream.clone() {
                Some(s) => {
                    let mut buf = [0u8; 1024];
                    let mut guard = s.lock().await;
                    select! {
                        read_result = guard.read(buf.as_mut_slice()) => {
                            match read_result {
                                Ok(bytes_read) => match self.stream_interpreter.decode::<ChimeraSessionCommand>(&buf.as_slice()[0..bytes_read]) {
                                    Ok(commands) => {
                                        for c in commands {
                                            match c {
                                                ChimeraSessionCommand::Terminate => exit_requested = true,
                                                ChimeraSessionCommand::Restart(command) => {
                                                    self.command = command;
                                                    proc.kill().await?
                                                },
                                            }
                                        }
                                    },
                                    Err(_) => {
                                        eprintln!("Invalid data detected. Closing socket.");
                                        stream = None
                                    },
                                },
                                Err(err) => {
                                    eprintln!("Socket will be terminated due to error in read: {}", err);
                                    stream = None
                                }
                            }
                        },
                        process_exit_result = proc.wait() => {
                            match process_exit_result {
                                Ok(status_code) => {
                                    println!("Process ended with: {}", status_code)
                                },
                                Err(err) => {
                                    eprintln!("Process errored: {}", err)
                                }
                            }

                            proc = self.start_child()?
                        }
                    }
                }
                None => select! {
                    listener_result = self.listener.accept() => {
                        let (socket, _) = listener_result?;
                        self.stream_interpreter.reset();
                        stream = Some(Arc::new(Mutex::new(socket)));
                    },
                    process_exit_result = proc.wait() => {
                        match process_exit_result {
                            Ok(status_code) => {
                                println!("Process ended with: {}", status_code)
                            },
                            Err(err) => {
                                eprintln!("Process errored: {}", err)
                            }
                        }

                        proc = self.start_child()?
                    }
                },
            }
        }

        println!("Session terminated as per requested.");

        Ok(())
    }
}
