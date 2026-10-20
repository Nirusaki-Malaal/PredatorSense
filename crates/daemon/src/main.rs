//! Native PHN16-71 daemon: safe Rust, one bounded server and one storage worker.
#![forbid(unsafe_code)]
mod backend;
mod server;
mod store;
use backend::Manager;
use predator_core::{Request, Response};
use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
fn main() {
    if let Err(error) = run() {
        eprintln!("predator-daemon: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.iter().any(|argument| argument == "--help") {
        println!("predator-daemon [--check] [--sysfs PATH] [--socket PATH] [--state PATH]");
        return Ok(());
    }
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--check" => index += 1,
            "--sysfs" | "--socket" | "--state" if index + 1 < arguments.len() => index += 2,
            argument => return Err(format!("Unknown option or missing value: {argument}").into()),
        }
    }
    let option = |key: &str, default: &str| {
        arguments
            .windows(2)
            .find(|pair| pair[0] == key)
            .map_or_else(|| PathBuf::from(default), |pair| PathBuf::from(&pair[1]))
    };
    let mut manager = Manager::new(
        option("--sysfs", "/sys"),
        option("--state", "/var/lib/predator-sense/preferences.json"),
    )
    .map_err(io::Error::other)?;
    if arguments.iter().any(|argument| argument == "--check") {
        // Read-only: do not restore or schedule writes during a diagnostic check.
        manager.refresh_read_only();
        println!("{}", serde_json::to_string_pretty(&manager.status())?);
        return Ok(());
    }
    let mut server = server::Server::bind(option("--socket", predator_core::SOCKET))?;
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(signal, stop.clone())?;
    }
    manager.restore();
    manager.refresh(true);
    let mut refresh = Instant::now();
    let mut discovery = Instant::now();
    let mut previous_boot = boottime();
    let mut previous_tick = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        server.turn(|request| {
            let result = match request {
                Request::Status => Ok(()),
                Request::Refresh => {
                    manager.refresh_read_only();
                    Ok(())
                }
                Request::Set { feature, value } => manager.set(&feature, &value),
                Request::ConfigureCurve { profile, points } => {
                    manager.configure_curve(&profile, &points)
                }
                Request::Cycle => manager.cycle(),
                Request::Turbo => manager.turbo(),
                Request::Restore => {
                    manager.restore();
                    if manager.status().errors.is_empty() {
                        Ok(())
                    } else {
                        Err(manager.status().errors.join("; "))
                    }
                }
            };
            match result {
                Ok(()) => Response::success(manager.status()),
                Err(error) => Response::error(error),
            }
        })?;
        let boot = boottime();
        if boot.saturating_sub(previous_boot) > previous_tick.elapsed() + Duration::from_secs(2) {
            manager.restore();
            manager.refresh(true);
        }
        previous_boot = boot;
        previous_tick = Instant::now();
        if refresh.elapsed() >= Duration::from_millis(500) {
            let changed = manager.check_power();
            manager.refresh(changed);
            refresh = Instant::now();
        }
        if discovery.elapsed() >= Duration::from_secs(10) {
            if manager.discover() {
                manager.restore();
                manager.refresh(true);
            }
            discovery = Instant::now();
        }
    }
    manager.finish();
    Ok(())
}
fn boottime() -> Duration {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::Boottime);
    Duration::new(time.tv_sec as u64, time.tv_nsec as u32)
}
