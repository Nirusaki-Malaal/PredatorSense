//! A bounded nonblocking framed server. Slow clients cannot stall other clients.
#![forbid(unsafe_code)]
use predator_core::{Request, Response, MAX_FRAME};
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    time::{Duration, Instant},
};
const MAX_CLIENTS: usize = 16;
const DEADLINE: Duration = Duration::from_millis(250);
struct Client {
    stream: UnixStream,
    input: Vec<u8>,
    output: Vec<u8>,
    sent: usize,
    started: Instant,
    handled: bool,
}
pub struct Server {
    listener: UnixListener,
    clients: Vec<Client>,
    path: PathBuf,
    _lock: fs::File,
}
impl Server {
    pub fn bind(path: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| io::Error::other("Invalid socket path"))?,
        )?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.try_lock().map_err(io::Error::other)?;
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if !metadata.file_type().is_socket() {
                return Err(io::Error::other("Refusing to replace a non-socket path"));
            }
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o660))?;
        Ok(Self {
            listener,
            clients: Vec::with_capacity(MAX_CLIENTS),
            path,
            _lock: lock,
        })
    }
    pub fn turn(&mut self, mut process: impl FnMut(Request) -> Response) -> io::Result<()> {
        let remaining = self
            .clients
            .iter()
            .map(|client| DEADLINE.saturating_sub(client.started.elapsed()))
            .min()
            .unwrap_or(Duration::from_millis(500));
        let timeout = Timespec {
            tv_sec: 0,
            tv_nsec: remaining.as_nanos().min(500_000_000) as _,
        };
        let ready = {
            let mut polls = Vec::with_capacity(MAX_CLIENTS + 1);
            polls.push(PollFd::new(&self.listener, PollFlags::IN));
            for client in &self.clients {
                polls.push(PollFd::new(
                    &client.stream,
                    if client.handled {
                        PollFlags::OUT
                    } else {
                        PollFlags::IN
                    },
                ));
            }
            if let Err(error) = poll(&mut polls, Some(&timeout)) {
                if error != rustix::io::Errno::INTR {
                    return Err(error.into());
                }
            }
            polls.iter().map(PollFd::revents).collect::<Vec<_>>()
        };
        for index in (0..self.clients.len()).rev() {
            let client = &mut self.clients[index];
            if client.started.elapsed() >= DEADLINE
                || !service(client, ready[index + 1], &mut process)
            {
                self.clients.swap_remove(index);
            }
        }
        if ready[0].contains(PollFlags::IN) {
            // Bound acceptance work as well as simultaneous clients.
            for _ in 0..MAX_CLIENTS {
                match self.listener.accept() {
                    Ok((stream, _)) if self.clients.len() < MAX_CLIENTS => {
                        stream.set_nonblocking(true)?;
                        self.clients.push(Client {
                            stream,
                            input: Vec::with_capacity(256),
                            output: Vec::new(),
                            sent: 0,
                            started: Instant::now(),
                            handled: false,
                        });
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }
}
fn service(
    client: &mut Client,
    flags: PollFlags,
    process: &mut impl FnMut(Request) -> Response,
) -> bool {
    if flags.intersects(PollFlags::ERR | PollFlags::NVAL) {
        return false;
    }
    if !client.handled && flags.intersects(PollFlags::IN | PollFlags::HUP) {
        let mut bytes = [0u8; 512];
        loop {
            match client.stream.read(&mut bytes) {
                Ok(0) => break,
                Ok(length) => {
                    if client.input.len() + length > MAX_FRAME + 4 {
                        return false;
                    }
                    client.input.extend_from_slice(&bytes[..length]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return false,
            }
        }
        if client.input.len() >= 4 {
            let length = u32::from_be_bytes(client.input[..4].try_into().unwrap()) as usize;
            let response = if length == 0 || length > MAX_FRAME {
                Some(Response::error("Invalid frame length"))
            } else if client.input.len() >= length + 4 {
                Some(if client.input.len() != length + 4 {
                    Response::error("One request is allowed per connection")
                } else {
                    match serde_json::from_slice(&client.input[4..]) {
                        Ok(request) => process(request),
                        Err(error) => Response::error(error),
                    }
                })
            } else {
                None
            };
            if let Some(response) = response {
                let mut data = serde_json::to_vec(&response).unwrap_or_default();
                if data.len() > MAX_FRAME {
                    data = serde_json::to_vec(&Response::error("Status exceeds protocol limit"))
                        .unwrap();
                }
                client
                    .output
                    .extend_from_slice(&(data.len() as u32).to_be_bytes());
                client.output.extend_from_slice(&data);
                client.handled = true;
            }
        }
    }
    if client.handled {
        while client.sent < client.output.len() {
            match client.stream.write(&client.output[client.sent..]) {
                Ok(0) => return false,
                Ok(length) => client.sent += length,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return true,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return false,
            }
        }
        return false;
    }
    !flags.contains(PollFlags::HUP)
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
