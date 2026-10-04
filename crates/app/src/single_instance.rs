//! One running copy per profile.
//!
//! Two copies would share the core's database and audio cache. The guard is
//! an exclusive lock on a file in the profile, which the system releases
//! when the process ends however it ends.
//!
//! A second launch is not an error: with the window closed to the tray,
//! starting the app again is how most people would ask for it back. So the
//! running copy listens on a loopback port, noted beside the lock, and the
//! second launch asks it to show itself before stepping aside.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The one thing a second launch says.
const SHOW: &[u8] = b"show\n";
const TIMEOUT: Duration = Duration::from_secs(2);

/// Held for the life of the process; dropping it releases the lock.
pub struct InstanceGuard {
    _lock: File,
    port_file: PathBuf,
}

pub enum Acquired {
    First(InstanceGuard),
    /// Another copy holds the profile. It has been asked to show itself,
    /// if it could be reached.
    AlreadyRunning,
}

pub fn acquire(lock_file: &Path) -> io::Result<Acquired> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_file)?;
    let port_file = lock_file.with_extension("port");
    match file.try_lock() {
        Ok(()) => Ok(Acquired::First(InstanceGuard {
            _lock: file,
            port_file,
        })),
        Err(TryLockError::WouldBlock) => {
            if let Err(error) = ask_to_show(&port_file) {
                // The log is not open yet at this point of a launch.
                eprintln!("the running copy could not be reached: {error}");
            }
            Ok(Acquired::AlreadyRunning)
        }
        Err(TryLockError::Error(error)) => Err(error),
    }
}

impl InstanceGuard {
    /// Listens for later launches and calls `on_show` for each, from a
    /// thread of its own.
    pub fn listen(&self, on_show: impl Fn() + Send + 'static) -> io::Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        std::fs::write(&self.port_file, listener.local_addr()?.port().to_string())?;
        std::thread::Builder::new()
            .name("instance".into())
            .spawn(move || {
                for mut stream in listener.incoming().flatten() {
                    let mut said = [0u8; SHOW.len()];
                    let _ = stream.set_read_timeout(Some(TIMEOUT));
                    // Anything else on the port is not a launch of this app.
                    if stream.read_exact(&mut said).is_ok() && said == SHOW {
                        on_show();
                    }
                }
            })?;
        Ok(())
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.port_file);
    }
}

fn ask_to_show(port_file: &Path) -> io::Result<()> {
    let port: u16 = std::fs::read_to_string(port_file)?
        .trim()
        .parse()
        .map_err(io::Error::other)?;
    let address = (Ipv4Addr::LOCALHOST, port).into();
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
    stream.write_all(SHOW)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn scratch(name: &str) -> io::Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!("spotified-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("instance.lock"))
    }

    #[test]
    fn a_second_copy_is_turned_away_until_the_first_ends() -> io::Result<()> {
        let lock_file = scratch("lock")?;
        let first = acquire(&lock_file)?;
        assert!(matches!(first, Acquired::First(_)));
        assert!(matches!(acquire(&lock_file)?, Acquired::AlreadyRunning));

        drop(first);
        assert!(matches!(acquire(&lock_file)?, Acquired::First(_)));
        Ok(())
    }

    #[test]
    fn a_second_launch_asks_the_first_to_show_itself() -> io::Result<()> {
        let lock_file = scratch("show")?;
        let Acquired::First(first) = acquire(&lock_file)? else {
            return Err(io::Error::other("the lock was already held"));
        };
        let (shown, asked) = mpsc::channel();
        first.listen(move || {
            let _ = shown.send(());
        })?;

        assert!(matches!(acquire(&lock_file)?, Acquired::AlreadyRunning));
        assert!(asked.recv_timeout(TIMEOUT).is_ok());
        Ok(())
    }
}
