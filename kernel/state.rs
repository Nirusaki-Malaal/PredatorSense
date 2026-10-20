// SPDX-License-Identifier: GPL-2.0-or-later
//! Runtime state. Foreign handles are atomic pointers; operations require STATE.
#![forbid(unsafe_code)]
pub(crate) use crate::thermal_state::ThermalState;
use crate::{
    ffi,
    policy::{CurvePoint, CurveProfile, FanMode, Hysteresis, DEFAULT_POINTS},
    transport::Endpoints,
};
use core::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};
#[derive(Clone, Copy)]
pub(crate) struct KeyboardState {
    pub(crate) per_zone: bool,
    pub(crate) mode: u8,
    pub(crate) speed: u8,
    pub(crate) brightness: u8,
    pub(crate) direction: u8,
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
    pub(crate) colors: [u64; 4],
    pub(crate) zone_brightness: i32,
}
impl KeyboardState {
    pub(crate) const fn empty() -> Self {
        Self {
            per_zone: false,
            mode: 0,
            speed: 0,
            brightness: 100,
            direction: 0,
            red: 0,
            green: 0,
            blue: 0,
            colors: [0; 4],
            zone_brightness: 100,
        }
    }
    pub(crate) fn valid(&self) -> bool {
        self.mode <= 7
            && self.speed <= 9
            && self.brightness <= 100
            && self.direction <= 2
            && (0..=100).contains(&self.zone_brightness)
            && self.colors.iter().all(|v| *v <= 0xffffff)
    }
}
pub(crate) struct State {
    pub(crate) endpoints: Endpoints,
    pub(crate) ready: bool,
    pub(crate) suspended: bool,
    pub(crate) platform: AtomicPtr<ffi::platform_device>,
    pub(crate) input: AtomicPtr<ffi::input_dev>,
    pub(crate) profile_device: AtomicPtr<ffi::device>,
    pub(crate) hwmon: AtomicPtr<ffi::device>,
    pub(crate) supported_sensors: u64,
    pub(crate) groups: u8,
    pub(crate) fan_mode: FanMode,
    pub(crate) curve_profile: CurveProfile,
    pub(crate) curve_points: [CurvePoint; 8],
    pub(crate) point_count: usize,
    pub(crate) cpu_speed: i32,
    pub(crate) gpu_speed: i32,
    pub(crate) cpu_hysteresis: Hysteresis,
    pub(crate) gpu_hysteresis: Hysteresis,
    pub(crate) maximum_profile: u8,
    pub(crate) previous_profile: u8,
    pub(crate) supported_profiles: u8,
    pub(crate) power_states: [ThermalState; 2],
    pub(crate) keyboard: KeyboardState,
    pub(crate) keyboard_valid: bool,
}
impl State {
    pub(crate) const fn new() -> Self {
        Self {
            endpoints: Endpoints::empty(),
            ready: false,
            suspended: false,
            platform: AtomicPtr::new(ptr::null_mut()),
            input: AtomicPtr::new(ptr::null_mut()),
            profile_device: AtomicPtr::new(ptr::null_mut()),
            hwmon: AtomicPtr::new(ptr::null_mut()),
            supported_sensors: 0,
            groups: 0,
            fan_mode: FanMode::Auto,
            curve_profile: CurveProfile::Conservative,
            curve_points: DEFAULT_POINTS,
            point_count: 5,
            cpu_speed: 0,
            gpu_speed: 0,
            cpu_hysteresis: Hysteresis::new(),
            gpu_hysteresis: Hysteresis::new(),
            maximum_profile: 5,
            previous_profile: 1,
            supported_profiles: 0,
            power_states: [ThermalState::defaults(false), ThermalState::defaults(true)],
            keyboard: KeyboardState::empty(),
            keyboard_valid: false,
        }
    }
    pub(crate) fn reset_hysteresis(&mut self) {
        self.cpu_hysteresis = Hysteresis::new();
        self.gpu_hysteresis = Hysteresis::new();
    }
}

impl State {
    pub(crate) fn platform(&self) -> *mut ffi::platform_device {
        self.platform.load(Ordering::Relaxed)
    }
    pub(crate) fn input(&self) -> *mut ffi::input_dev {
        self.input.load(Ordering::Relaxed)
    }
    pub(crate) fn profile_device(&self) -> *mut ffi::device {
        self.profile_device.load(Ordering::Relaxed)
    }
}
