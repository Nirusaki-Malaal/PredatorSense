//! Bounded asynchronous preference writes; firmware operations never wait for fsync.
#![forbid(unsafe_code)]
use predator_core::validate;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub type Preferences = BTreeMap<String, String>;
fn feature(key: &str) -> &str {
    key.strip_suffix("_ac")
        .or_else(|| key.strip_suffix("_battery"))
        .unwrap_or(key)
}
pub struct Store {
    sender: Option<SyncSender<Preferences>>,
    results: Receiver<Result<(), String>>,
    worker: Option<JoinHandle<()>>,
}
impl Store {
    pub fn open(path: PathBuf) -> (Self, Preferences, Option<String>) {
        let loaded = (|| -> Result<Preferences, String> {
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Preferences::new())
                }
                Err(error) => return Err(error.to_string()),
            };
            let mut bytes = Vec::with_capacity(1024);
            file.take(8193)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            if bytes.len() > 8192 {
                return Err("Preferences exceed 8192 bytes".into());
            }
            let saved: Preferences =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            for (key, value) in &saved {
                if feature(key) == "battery_calibration" {
                    return Err("Calibration must not be replayed at startup".into());
                }
                validate(feature(key), value)?;
            }
            Ok(saved)
        })();
        let (saved, warning, mut preserve_bad_file) = match loaded {
            Ok(saved) => (saved, None, false),
            Err(error) => (Preferences::new(), Some(format!("Preferences could not be loaded: {error}. Hardware controls remain available; the original file is preserved.")), true),
        };
        let (sender, jobs) = mpsc::sync_channel::<Preferences>(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            while let Ok(mut preferences) = jobs.recv() {
                // Merge rapid edits for at most 40 ms; the channel holds one map.
                let deadline = std::time::Instant::now() + Duration::from_millis(40);
                while let Some(remaining) =
                    deadline.checked_duration_since(std::time::Instant::now())
                {
                    match jobs.recv_timeout(remaining) {
                        Ok(latest) => preferences = latest,
                        Err(_) => break,
                    }
                }
                let result = persist(&path, &preferences, &mut preserve_bad_file);
                if result.is_err() {
                    let _ = results.try_send(result);
                }
            }
        });
        (
            Self {
                sender: Some(sender),
                results: receiver,
                worker: Some(worker),
            },
            saved,
            warning,
        )
    }
    pub fn enqueue(&self, saved: &Preferences) -> bool {
        self.sender
            .as_ref()
            .is_some_and(|sender| sender.try_send(saved.clone()).is_ok())
    }
    pub fn errors(&self) -> impl Iterator<Item = String> + '_ {
        self.results.try_iter().filter_map(Result::err)
    }
    pub fn finish(&mut self, saved: &Preferences) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(saved.clone());
            drop(sender);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn persist(
    path: &std::path::Path,
    saved: &Preferences,
    preserve_bad_file: &mut bool,
) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid preference path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = path.with_extension(format!("{}.{}.tmp", std::process::id(), stamp));
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        let bytes = serde_json::to_vec_pretty(saved)?;
        if bytes.len() > 8192 {
            return Err(std::io::Error::other("Preferences exceed storage limit"));
        }
        file.write_all(&bytes)?;
        file.sync_all()?;
        if *preserve_bad_file && path.exists() {
            fs::rename(path, path.with_extension(format!("corrupt.{stamp}")))?;
            *preserve_bad_file = false;
        }
        fs::rename(&temporary, path)?;
        File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| error.to_string())
}
