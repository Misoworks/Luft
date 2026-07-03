use luft_ipc::{SHELL_SOCKET_ENV, ShellControlRequest, ensure_socket_parent, read_shell_control};
use std::{env, fs, io, os::unix::net::UnixListener, path::PathBuf};

#[derive(Debug)]
pub struct ShellControlServer {
    listener: UnixListener,
    path: PathBuf,
}

impl ShellControlServer {
    pub fn bind_from_env() -> io::Result<Option<Self>> {
        let Some(path) = env::var_os(SHELL_SOCKET_ENV).map(PathBuf::from) else {
            return Ok(None);
        };

        ensure_socket_parent(&path)?;
        remove_stale_socket(&path)?;
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        Ok(Some(Self { listener, path }))
    }

    pub fn drain(&self) -> io::Result<Vec<ShellControlRequest>> {
        let mut requests = Vec::new();
        loop {
            match self.listener.accept() {
                Ok((mut stream, _)) => requests.push(read_shell_control(&mut stream)?),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(requests),
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for ShellControlServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn remove_stale_socket(path: &PathBuf) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
