use std::{collections::HashMap, env, ffi::OsString, fs::File, path::{Path, PathBuf}, process::Stdio, sync::Arc};

use tokio::{
    select,
    io::AsyncReadExt,
    net::{UnixListener, UnixStream},
    process::{Child, Command},
    sync::Mutex,
};

use users::os::unix::UserExt;

use chrono::Local;

use crate::{command::*, stream_command::StreamCommand};

pub struct Session {
    listener: UnixListener,
    sock_addr: OsString,
    stream_interpreter: StreamCommand,
    command: SessionExecutable,
}

fn create_log_file(name: &str) -> Option<File> {
    let current_user = users::get_user_by_uid(users::get_current_uid());
    let home_pathbuf = match current_user {
        Some(user) => PathBuf::from(user.home_dir()),
        None => match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home.as_str()),
            Err(_) => return None
        }
    };

    if !home_pathbuf.exists() {
        return None
    }

    let logsdir_pathbuf = home_pathbuf.join("chimera_session");

    if !logsdir_pathbuf.clone().exists() {
        if std::fs::create_dir(logsdir_pathbuf.clone()).is_err() {
            return None
        }
    }

    match File::create(logsdir_pathbuf.join(format!("{}_{}.log", name, Local::now().format("%Y%m%d_%H%M%S")).as_str())) {
        Ok(file) => Some(file),
        Err(_) => None
    }
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
            .stdout(match create_log_file("stdout") {
                Some(output_file) => Stdio::from(output_file),
                None => Stdio::null()
            })
            .stderr(match create_log_file("stderr") {
                Some(output_file) => Stdio::from(output_file),
                None => Stdio::null()
            })
            .stdin(Stdio::null())
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
                                        self.stream_interpreter.reset();
                                        stream = None
                                    },
                                },
                                Err(err) => {
                                    eprintln!("Socket will be terminated due to error in read: {}", err);
                                    self.stream_interpreter.reset();
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
