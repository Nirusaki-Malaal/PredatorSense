//! PHN16-71 sysfs access, bounded status cache and per-power preferences.
#![forbid(unsafe_code)]
use crate::store::{Preferences, Store};
use predator_core::{validate, Status};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
const FEATURES: &[&str] = &[
    "profile",
    "backlight_timeout",
    "battery_limiter",
    "battery_calibration",
    "boot_animation_sound",
    "lcd_override",
    "usb_charging",
    "fan_speed",
    "thermal_fan_mode",
    "fan_curve_profile",
    "fan_curve_points",
    "per_zone_mode",
    "four_zone_mode",
];
const THERMAL: &[&str] = &[
    "profile",
    "fan_speed",
    "thermal_fan_mode",
    "fan_curve_profile",
    "fan_curve_points",
    "backlight_timeout",
];
pub struct Manager {
    root: PathBuf,
    paths: BTreeMap<String, PathBuf>,
    cached: Status,
    saved: Preferences,
    store: Store,
    dirty: bool,
    power: String,
    previous_profile: String,
    warning: Option<String>,
    restore_errors: Vec<String>,
}
fn read(path: &Path) -> Result<String, String> {
    for attempt in 0..4 {
        let mut value = String::new();
        let result = File::open(path).and_then(|file| file.take(1025).read_to_string(&mut value));
        match result {
            Ok(_) if value.len() <= 1024 => return Ok(value.trim().into()),
            Ok(_) => return Err(format!("{}: response too long", path.display())),
            Err(error) if error.raw_os_error() == Some(16) && attempt < 3 => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    }
    unreachable!()
}
impl Manager {
    pub fn new(root: PathBuf, state: PathBuf) -> Result<Self, String> {
        let model = read(&root.join("class/dmi/id/product_name"))?;
        let vendor = read(&root.join("class/dmi/id/sys_vendor"))?;
        if model != "Predator PHN16-71" || vendor != "Acer" {
            return Err(format!("Unsupported hardware: {vendor} {model}; PredatorSense is specialized exclusively for Acer Predator PHN16-71"));
        }
        let (store, saved, warning) = Store::open(state);
        let mut manager = Self {
            root,
            paths: BTreeMap::new(),
            cached: Status {
                model,
                backend: "PHN16-71 WMI".into(),
                ..Default::default()
            },
            saved,
            store,
            dirty: false,
            power: String::new(),
            previous_profile: "balanced".into(),
            warning,
            restore_errors: Vec::new(),
        };
        manager.power = manager.read_power().into();
        manager.discover();
        Ok(manager)
    }
    pub fn discover(&mut self) -> bool {
        let mut paths = BTreeMap::new();
        for feature in FEATURES {
            let path = if *feature == "profile" {
                self.root.join("firmware/acpi/platform_profile")
            } else {
                self.root
                    .join("devices/platform/acer-wmi")
                    .join(if matches!(*feature, "four_zone_mode" | "per_zone_mode") {
                        "four_zoned_kb"
                    } else {
                        "predator_sense"
                    })
                    .join(feature)
            };
            if path.is_file() {
                paths.insert((*feature).into(), path);
            }
        }
        let changed = paths != self.paths;
        self.paths = paths;
        self.cached.supported = self.paths.keys().cloned().collect();
        self.cached.choices = read(&self.root.join("firmware/acpi/platform_profile_choices"))
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        changed
    }
    pub fn refresh_read_only(&mut self) {
        self.cached.errors = self
            .warning
            .iter()
            .chain(self.restore_errors.iter())
            .cloned()
            .collect();
        for (feature, path) in &self.paths {
            match read(path) {
                Ok(value) => {
                    self.cached.values.insert(feature.clone(), value);
                }
                Err(error) => self.cached.errors.push(error),
            }
        }
    }
    pub fn refresh(&mut self, full: bool) {
        let previous = self.cached.values.clone();
        self.cached.errors = self
            .warning
            .iter()
            .chain(self.restore_errors.iter())
            .cloned()
            .collect();
        for (feature, path) in &self.paths {
            // Live profile and fan policy are cheap. RGB reads require five
            // firmware transactions and run only on explicit/full refresh.
            if !full
                && !matches!(
                    feature.as_str(),
                    "profile"
                        | "fan_speed"
                        | "thermal_fan_mode"
                        | "fan_curve_profile"
                        | "fan_curve_points"
                )
            {
                continue;
            }
            match read(path) {
                Ok(value) => {
                    self.cached.values.insert(feature.clone(), value);
                }
                Err(error) => self.cached.errors.push(error),
            }
        }
        let storage_errors: Vec<_> = self.store.errors().collect();
        if !storage_errors.is_empty() {
            // A failed background flush must remain eligible for retry even if
            // the user makes no further edits.
            self.dirty = true;
        }
        self.cached.errors.extend(storage_errors);
        self.cached.errors.truncate(16);
        // Also capture settings changed by the physical mode key or direct sysfs
        // writes. Never observe the new supply under the previous power key.
        if self.read_power() == self.power {
            for feature in THERMAL {
                if let Some(value) = self.cached.values.get(*feature) {
                    let key = format!("{feature}_{}", self.power);
                    if self.saved.get(&key) != Some(value)
                        && (!self.saved.contains_key(&key)
                            || previous.get(*feature).is_some_and(|old| old != value))
                    {
                        self.saved.insert(key, value.clone());
                        self.dirty = true;
                    }
                }
            }
        }
        self.schedule_save();
    }
    pub fn status(&self) -> Status {
        let mut status = self.cached.clone();
        status.saved = self.saved.clone();
        status
    }
    fn schedule_save(&mut self) {
        if self.dirty && self.store.enqueue(&self.saved) {
            self.dirty = false;
        }
    }
    fn write(&self, feature: &str, value: &str) -> Result<(), String> {
        validate(feature, value)?;
        if feature == "profile" && !self.cached.choices.iter().any(|choice| choice == value) {
            return Err("Profile unsupported by firmware".into());
        }
        let path = self
            .paths
            .get(feature)
            .ok_or_else(|| format!("{feature} is not available"))?;
        for attempt in 0..4 {
            let result = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .and_then(|mut file| file.write_all(format!("{value}\n").as_bytes()));
            match result {
                Ok(()) => return Ok(()),
                Err(error) if error.raw_os_error() == Some(16) && attempt < 3 => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(error) => return Err(format!("{}: {error}", path.display())),
            }
        }
        unreachable!()
    }
    fn remember(&mut self, feature: &str, value: &str) {
        self.cached.values.insert(feature.into(), value.into());
        if feature == "battery_calibration" {
            return;
        }
        if matches!(feature, "four_zone_mode" | "per_zone_mode") {
            self.saved.remove("four_zone_mode");
            self.saved.remove("per_zone_mode");
        }
        let key = if THERMAL.contains(&feature) {
            format!("{feature}_{}", self.read_power())
        } else {
            feature.into()
        };
        self.saved.insert(key, value.into());
        self.dirty = true;
    }
    pub fn set(&mut self, feature: &str, value: &str) -> Result<(), String> {
        self.write(feature, value)?;
        self.remember(feature, value);
        self.schedule_save();
        Ok(())
    }
    pub fn configure_curve(&mut self, profile: &str, points: &str) -> Result<(), String> {
        validate("fan_curve_points", points)?;
        validate("fan_curve_profile", profile)?;
        for (feature, value) in [
            ("fan_curve_points", points),
            ("fan_curve_profile", profile),
            ("thermal_fan_mode", "curve"),
        ] {
            self.write(feature, value)?;
            self.remember(feature, value);
        }
        self.schedule_save();
        Ok(())
    }
    fn read_power(&self) -> &'static str {
        if let Ok(devices) = fs::read_dir(self.root.join("class/power_supply")) {
            for device in devices.flatten() {
                if matches!(
                    read(&device.path().join("type"))
                        .unwrap_or_default()
                        .as_str(),
                    "Mains" | "USB" | "USB_C" | "USB_PD"
                ) && read(&device.path().join("online")).is_ok_and(|value| value == "1")
                {
                    return "ac";
                }
            }
        }
        "battery"
    }
    pub fn check_power(&mut self) -> bool {
        let power = self.read_power();
        if power == self.power {
            return false;
        }
        self.power = power.into();
        self.restore();
        true
    }
    pub fn restore(&mut self) {
        self.restore_errors.clear();
        self.discover();
        // Curves/points and fixed percentages precede fan mode. Profile is last,
        // so firmware policy uses the complete restored configuration.
        for feature in [
            "fan_curve_points",
            "fan_curve_profile",
            "fan_speed",
            "thermal_fan_mode",
            "backlight_timeout",
            "profile",
            "battery_limiter",
            "boot_animation_sound",
            "lcd_override",
            "usb_charging",
            "four_zone_mode",
            "per_zone_mode",
        ] {
            let key = if THERMAL.contains(&feature) {
                format!("{feature}_{}", self.power)
            } else {
                feature.into()
            };
            if let Some(value) = self.saved.get(&key).cloned() {
                match self.write(feature, &value) {
                    Ok(()) => {
                        self.cached.values.insert(feature.into(), value);
                    }
                    Err(error) => self.restore_errors.push(error),
                }
            }
        }
        self.cached.errors = self.restore_errors.clone();
    }
    pub fn cycle(&mut self) -> Result<(), String> {
        let current = read(self.paths.get("profile").ok_or("No profile interface")?)?;
        let choices: Vec<_> = self
            .cached
            .choices
            .iter()
            .filter(|choice| {
                self.read_power() == "ac" || matches!(choice.as_str(), "low-power" | "balanced")
            })
            .cloned()
            .collect();
        if choices.is_empty() {
            return Err("No profiles available".into());
        }
        let next = choices
            .iter()
            .position(|choice| choice == &current)
            .map_or(0, |index| (index + 1) % choices.len());
        self.set("profile", &choices[next])
    }
    pub fn turbo(&mut self) -> Result<(), String> {
        let current = read(self.paths.get("profile").ok_or("No profile interface")?)?;
        if current == "performance" {
            self.set("profile", &self.previous_profile.clone())
        } else if self.read_power() != "ac" {
            Err("Turbo requires AC power".into())
        } else {
            self.previous_profile = current;
            self.set("profile", "performance")
        }
    }
    pub fn finish(&mut self) {
        self.store.finish(&self.saved);
    }
}
