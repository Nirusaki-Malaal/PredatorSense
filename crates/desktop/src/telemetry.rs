//! Read-only dashboard sampling. Histories and subprocess output are bounded.
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
#[derive(Default, Clone)]
pub struct Snapshot {
    pub cpu_name: String,
    pub gpu_name: String,
    pub gpu_driver: String,
    pub os: String,
    pub kernel: String,
    pub cpu_usage: f64,
    pub ram_usage: f64,
    pub ram_total_gib: f64,
    pub cpu_temperature: Option<f64>,
    pub gpu_temperature: Option<f64>,
    pub gpu_usage: Option<f64>,
    pub cpu_fan: Option<u32>,
    pub gpu_fan: Option<u32>,
    pub battery: Option<u32>,
    pub battery_status: String,
    pub remaining: String,
}
fn read(path: impl AsRef<Path>) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(65536).read_to_string(&mut text).ok()?;
    Some(text.trim().into())
}
fn number(path: impl AsRef<Path>) -> Option<f64> {
    read(path)?.parse().ok()
}
fn command(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut text = String::new();
                child
                    .stdout
                    .take()?
                    .take(4096)
                    .read_to_string(&mut text)
                    .ok()?;
                return Some(text);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
pub struct Sampler {
    previous_cpu: Option<(u64, u64)>,
    static_info: Snapshot,
    gpu_device: Option<PathBuf>,
}
impl Sampler {
    pub fn new() -> Self {
        let cpu_name = read("/proc/cpuinfo")
            .and_then(|text| {
                text.lines().find_map(|line| {
                    line.strip_prefix("model name").and_then(|value| {
                        value.split_once(':').map(|(_, value)| value.trim().into())
                    })
                })
            })
            .unwrap_or_else(|| "Unknown CPU".into());
        let os = read("/etc/os-release")
            .and_then(|text| {
                text.lines().find_map(|line| {
                    line.strip_prefix("PRETTY_NAME=")
                        .map(|value| value.trim_matches('"').into())
                })
            })
            .unwrap_or_else(|| "Linux".into());
        let kernel = read("/proc/sys/kernel/osrelease").unwrap_or_default();
        let mut info = Snapshot {
            cpu_name,
            os,
            kernel,
            ..Snapshot::default()
        };
        let mut gpu_device = None;
        if let Ok(cards) = fs::read_dir("/sys/class/drm") {
            for card in cards.flatten() {
                let name = card.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("card")
                    && !name.contains('-')
                    && card.path().join("device/vendor").exists()
                {
                    let device = card.path().join("device");
                    let vendor = read(device.join("vendor")).unwrap_or_default();
                    let preferred = vendor == "0x10de" || vendor == "0x1002";
                    if gpu_device.is_none() || preferred {
                        info.gpu_name = match vendor.as_str() {
                            "0x10de" => "NVIDIA GPU",
                            "0x1002" => "AMD GPU",
                            "0x8086" => "Intel GPU",
                            _ => "GPU",
                        }
                        .into();
                        if let Ok(path) = device.canonicalize() {
                            if let Some(address) = path.file_name().and_then(|name| name.to_str()) {
                                if let Some(name) = command("/usr/bin/lspci", &["-s", address]) {
                                    info.gpu_name = name.trim().into();
                                }
                            }
                        }
                        let driver = fs::read_link(device.join("driver"))
                            .ok()
                            .and_then(|path| {
                                path.file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                            })
                            .unwrap_or_default();
                        info.gpu_driver =
                            read(format!("/sys/module/{driver}/version")).unwrap_or(driver);
                        gpu_device = Some(device);
                    }
                }
            }
        }
        Self {
            previous_cpu: None,
            static_info: info,
            gpu_device,
        }
    }
    pub fn sample(&mut self) -> Snapshot {
        let mut result = self.static_info.clone();
        if let Some(stat) = read("/proc/stat") {
            if let Some(line) = stat.lines().next() {
                let values: Vec<u64> = line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|value| value.parse().ok())
                    .take(8)
                    .collect();
                if values.len() >= 4 {
                    let total: u64 = values.iter().sum();
                    let idle = values[3] + values.get(4).copied().unwrap_or(0);
                    if let Some((previous_total, previous_idle)) = self.previous_cpu {
                        let elapsed = total.saturating_sub(previous_total);
                        if elapsed > 0 {
                            result.cpu_usage = 100.0
                                * (elapsed - idle.saturating_sub(previous_idle).min(elapsed))
                                    as f64
                                / elapsed as f64;
                        }
                    }
                    self.previous_cpu = Some((total, idle));
                }
            }
        }
        if let Some(memory) = read("/proc/meminfo") {
            let value = |name: &str| {
                memory.lines().find_map(|line| {
                    line.strip_prefix(name)
                        .and_then(|value| value.split_whitespace().next()?.parse::<f64>().ok())
                })
            };
            if let (Some(total), Some(available)) = (value("MemTotal:"), value("MemAvailable:")) {
                result.ram_total_gib = total / 1048576.0;
                result.ram_usage = 100.0 * (total - available) / total;
            }
        }
        if let Ok(monitors) = fs::read_dir("/sys/class/hwmon") {
            for monitor in monitors.flatten() {
                let path = monitor.path();
                let name = read(path.join("name")).unwrap_or_default();
                match name.as_str() {
                    "acer" => {
                        result.cpu_temperature =
                            number(path.join("temp1_input")).map(|value| value / 1000.0);
                        result.gpu_temperature =
                            number(path.join("temp2_input")).map(|value| value / 1000.0);
                        result.cpu_fan = number(path.join("fan1_input")).map(|value| value as u32);
                        result.gpu_fan = number(path.join("fan2_input")).map(|value| value as u32);
                    }
                    "coretemp" | "k10temp" if result.cpu_temperature.is_none() => {
                        result.cpu_temperature =
                            number(path.join("temp1_input")).map(|value| value / 1000.0)
                    }
                    "amdgpu" if result.gpu_temperature.is_none() => {
                        result.gpu_temperature =
                            number(path.join("temp1_input")).map(|value| value / 1000.0)
                    }
                    _ => {}
                }
            }
        }
        if let Some(device) = &self.gpu_device {
            result.gpu_usage = number(device.join("gpu_busy_percent"));
        }
        if self
            .static_info
            .gpu_name
            .to_ascii_lowercase()
            .contains("nvidia")
        {
            if let Some(output) = command(
                "/usr/bin/nvidia-smi",
                &[
                    "--query-gpu=name,temperature.gpu,utilization.gpu,driver_version",
                    "--format=csv,noheader,nounits",
                ],
            ) {
                let values: Vec<_> = output
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .collect();
                if values.len() == 4 {
                    result.gpu_name = values[0].into();
                    result.gpu_temperature = values[1].parse().ok();
                    result.gpu_usage = values[2].parse().ok();
                    result.gpu_driver = values[3].into();
                }
            }
        }
        if let Ok(supplies) = fs::read_dir("/sys/class/power_supply") {
            for supply in supplies.flatten() {
                let path = supply.path();
                if read(path.join("type")).as_deref() != Some("Battery") {
                    continue;
                }
                result.battery = number(path.join("capacity")).map(|value| value as u32);
                result.battery_status = read(path.join("status")).unwrap_or_default();
                let charging = result.battery_status == "Charging";
                let energy =
                    number(path.join("energy_now")).or_else(|| number(path.join("charge_now")));
                let full =
                    number(path.join("energy_full")).or_else(|| number(path.join("charge_full")));
                let power =
                    number(path.join("power_now")).or_else(|| number(path.join("current_now")));
                if let (Some(energy), Some(full), Some(power)) = (energy, full, power) {
                    if power > 0.0 {
                        let minutes = (if charging { full - energy } else { energy } / power * 60.0)
                            .max(0.0) as u64;
                        result.remaining = format!("{} h {} min", minutes / 60, minutes % 60);
                    }
                }
                break;
            }
        }
        result
    }
}
