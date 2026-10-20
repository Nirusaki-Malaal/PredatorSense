//! Launch sibling executables and always reap child processes.
use std::{
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
pub fn launch(program: &str) -> Result<(), String> {
    let previous = ACTIVE.fetch_add(1, Ordering::AcqRel);
    if previous >= 4 {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
        return Err("A launch is already pending".into());
    }
    let executable = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(program)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| program.into());
    match Command::new(executable).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
                ACTIVE.fetch_sub(1, Ordering::AcqRel);
            });
            Ok(())
        }
        Err(error) => {
            ACTIVE.fetch_sub(1, Ordering::AcqRel);
            Err(error.to_string())
        }
    }
}
