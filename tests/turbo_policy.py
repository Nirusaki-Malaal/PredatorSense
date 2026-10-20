#!/usr/bin/env python3
"""Execute the real gaming controller against a recording firmware transport."""
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
source = r'''
#![allow(dead_code, non_camel_case_types, unused_unsafe)]
extern crate self as kernel;
#[macro_export] macro_rules! pr_info { ($($argument:tt)*) => { println!($($argument)*) }; }
mod ffi {
    pub struct platform_device;
    pub struct input_dev;
    pub struct device;
    pub fn platform_profile_notify(_: *mut device) {}
}
mod parameters { pub fn cycle_profiles() -> bool { true } }
mod transport {
    use std::sync::{Mutex, atomic::{AtomicU8, Ordering}};
    pub type Result<T> = core::result::Result<T, i32>;
    pub const ACTION: usize = 0;
    pub const GAMING: usize = 1;
    pub const BATTERY: usize = 2;
    pub struct Endpoints {
        pub profile: AtomicU8,
        pub calls: Mutex<Vec<(u32, Vec<u8>)>>,
    }
    impl Endpoints {
        pub const fn empty() -> Self { Self { profile: AtomicU8::new(1), calls: Mutex::new(Vec::new()) } }
        pub fn procedure(&self, _: usize, method: u32, input: &[u8]) -> Result<()> {
            self.calls.lock().unwrap().push((method, input.to_vec())); Ok(())
        }
        pub fn invoke(&self, _: usize, _: u32, _: &[u8], _: usize) -> Result<Reply> { Ok(Reply) }
        pub fn number64(&self, _: usize, _: u32, _: u64) -> Result<u64> { Ok(1) }
        pub fn system_info(&self, _: u32) -> Result<u64> { Ok(65 << 8) }
        pub fn misc_get(&self, _: u8) -> Result<u8> { Ok(self.profile.load(Ordering::Relaxed)) }
        pub fn misc_set(&self, _: u8, value: u8) -> Result<()> { self.profile.store(value, Ordering::Relaxed); Ok(()) }
    }
    pub struct Reply;
    impl Reply {
        pub fn bytes(&self) -> &[u8] { &[0; 16] }
        pub fn number(&self) -> Result<u64> { Ok(0) }
    }
}
#[path = "@ROOT@/kernel/policy.rs"] mod policy;
#[path = "@ROOT@/kernel/thermal_state.rs"] mod thermal_state;
#[path = "@ROOT@/kernel/state.rs"] mod state;
#[path = "@ROOT@/kernel/gaming.rs"] mod gaming;
fn commands(state: &state::State) -> Vec<u64> {
    state.endpoints.calls.lock().unwrap().iter().filter(|(method, _)| *method == 14)
        .map(|(_, payload)| u64::from_le_bytes(payload.as_slice().try_into().unwrap())).collect()
}
#[test]
fn automatic_cooling_survives_every_profile_including_turbo() {
    let mut state = state::State::new();
    for profile in [0, 1, 4, 5, 6] {
        state.endpoints.calls.lock().unwrap().clear();
        gaming::set_profile(&mut state, profile).unwrap();
        assert_eq!(commands(&state), vec![0x410009], "profile {profile}");
        assert_eq!((state.cpu_speed, state.gpu_speed), (0, 0));
    }
}
#[test]
fn fixed_percentages_survive_turbo_entry_and_exit() {
    let mut state = state::State::new();
    state.fan_mode = policy::FanMode::Fixed;
    state.cpu_speed = 40;
    state.gpu_speed = 55;
    for profile in [5, 1] {
        gaming::set_profile(&mut state, profile).unwrap();
        assert_eq!((state.cpu_speed, state.gpu_speed), (40, 55));
    }
    assert!(!commands(&state).contains(&0x820009));
}
#[test]
fn turbo_does_not_skip_custom_curve_updates() {
    let mut state = state::State::new();
    state.fan_mode = policy::FanMode::Curve;
    gaming::set_profile(&mut state, 5).unwrap();
    assert!(!state.endpoints.calls.lock().unwrap().is_empty());
    assert!(!commands(&state).contains(&0x820009));
    state.endpoints.calls.lock().unwrap().clear();
    gaming::curve_tick(&mut state).unwrap();
    assert!(!state.endpoints.calls.lock().unwrap().is_empty());
}
'''.replace('@ROOT@', str(ROOT))
with tempfile.TemporaryDirectory(prefix='predator-turbo-policy-') as directory:
    directory = pathlib.Path(directory)
    (directory / 'test.rs').write_text(source)
    subprocess.run(['/usr/bin/rustc', '--edition=2021', '--test', directory / 'test.rs', '-o', directory / 'test'], check=True)
    subprocess.run([directory / 'test'], check=True)
