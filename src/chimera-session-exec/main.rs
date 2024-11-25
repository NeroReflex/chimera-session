use std::io::{self, Error};
use std::path::Path;

#[tokio::main]
async fn main() -> io::Result<()> {
    let uid = users::get_current_uid();

    let run_basedir = format!("/run/user/{}", uid);

    if !Path::new(run_basedir.as_str()).exists() {
        return Err(Error::new(
            io::ErrorKind::NotADirectory,
            format!("directory {} does not exists", run_basedir),
        ));
    }

    for idx in 0..100 {
        let socket_path = format!("{}/login_ng-{}.sock", run_basedir, idx);
        let socket_fs_path = Path::new(socket_path.as_str());
        if !socket_fs_path.exists() {
            let mut session = chimera_session::session::Session::new(socket_fs_path)?;

            return session.run().await;
        }
    }

    Err(Error::new(
        std::io::ErrorKind::AddrInUse,
        "Could not create a new socket, every available name taken.",
    ))
}
