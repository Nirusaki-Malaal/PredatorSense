#![forbid(unsafe_code)]
mod launcher;
use ksni::blocking::TrayMethods;
struct Tray;
fn open() {
    if let Err(error) = launcher::launch("predator-sense") {
        eprintln!("Cannot open UI: {error}");
    }
}
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "predator-sense".into()
    }
    fn title(&self) -> String {
        "Predator Sense".into()
    }
    fn icon_name(&self) -> String {
        "predator-sense".into()
    }
    fn activate(&mut self, _: i32, _: i32) {
        open();
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Open Predator Sense".into(),
                activate: Box::new(|_| open()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Cycle performance mode".into(),
                activate: Box::new(|_| {
                    if let Err(e) = predator_core::call(
                        &predator_core::socket_path(),
                        &predator_core::Request::Cycle,
                    ) {
                        eprintln!("{e}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit tray".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}
fn main() {
    // One tray per user; no GTK initialization or renderer in this process.
    let path = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("predator-sense-tray.lock");
    let lock = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    if lock.try_lock().is_err() {
        return;
    }
    match Tray.spawn() {
        Ok(handle) => {
            let _handle = handle;
            loop {
                std::thread::park();
            }
        }
        Err(e) => {
            eprintln!("Tray unavailable: {e}. Your desktop needs StatusNotifierItem support (GNOME: AppIndicator extension).");
            std::process::exit(1);
        }
    }
}
