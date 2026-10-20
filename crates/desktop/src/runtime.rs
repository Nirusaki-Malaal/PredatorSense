//! Configure allocator retention before GTK or background threads initialize.
//!
//! Re-exec applies settings only to this process, without unsafe mutation of
//! the environment after threads have started. Graphics selection is untouched.
#![forbid(unsafe_code)]
use std::{env, io, os::unix::process::CommandExt, process::Command};

pub fn prepare() -> io::Result<()> {
    // glibc otherwise retains temporary font/layout and rendering allocations.
    // Respect allocator settings supplied by the caller. Other libc runtimes
    // ignore this variable; no allocator-specific FFI is introduced.
    if env::var_os("MALLOC_TRIM_THRESHOLD_").is_some() {
        return Ok(());
    }
    let error = Command::new(env::current_exe()?)
        .args(env::args_os().skip(1))
        .env("MALLOC_TRIM_THRESHOLD_", "0")
        .exec();
    Err(error)
}
