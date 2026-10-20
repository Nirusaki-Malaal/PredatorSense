// SPDX-License-Identifier: GPL-2.0-or-later
//! PHN16-71 Linux interfaces. Unsafe code is limited to kernel C callbacks/ownership.
use crate::{
    ffi, gaming, keymap, policy,
    state::State,
    sysfs,
    transport::{self, pointer_error, Result, EVENTS},
};
use core::{
    ffi::{c_int, c_long, c_void},
    ptr,
    sync::atomic::{AtomicU64, Ordering},
};
static PROFILE_CHOICES: AtomicU64 = AtomicU64::new(0);
static SUPPORTED_SENSORS: AtomicU64 = AtomicU64::new(0);
unsafe extern "C" fn profile_probe(_: *mut c_void, choices: *mut core::ffi::c_ulong) -> c_int {
    unsafe {
        *choices = PROFILE_CHOICES.load(Ordering::Relaxed) as _;
    }
    0
}
unsafe extern "C" fn profile_get(
    _: *mut ffi::device,
    value: *mut ffi::platform_profile_option,
) -> c_int {
    match crate::with_state(|state| state.endpoints.misc_get(11)) {
        Ok(profile) => {
            let profile = match profile {
                6 => ffi::PLATFORM_PROFILE_LOW_POWER,
                0 => ffi::PLATFORM_PROFILE_QUIET,
                1 => ffi::PLATFORM_PROFILE_BALANCED,
                4 => ffi::PLATFORM_PROFILE_BALANCED_PERFORMANCE,
                5 => ffi::PLATFORM_PROFILE_PERFORMANCE,
                _ => return -95,
            };
            unsafe {
                *value = profile;
            }
            0
        }
        Err(error) => error,
    }
}
unsafe extern "C" fn profile_set(
    _: *mut ffi::device,
    value: ffi::platform_profile_option,
) -> c_int {
    let profile = match value {
        ffi::PLATFORM_PROFILE_LOW_POWER => 6,
        ffi::PLATFORM_PROFILE_QUIET => 0,
        ffi::PLATFORM_PROFILE_BALANCED => 1,
        ffi::PLATFORM_PROFILE_BALANCED_PERFORMANCE => 4,
        ffi::PLATFORM_PROFILE_PERFORMANCE => 5,
        _ => return -95,
    };
    crate::with_state(|state| {
        gaming::set_profile(state, profile)?;
        Ok(())
    })
    .map_or_else(|error| error, |_| 0)
}
const PROFILE_OPS: &'static ffi::platform_profile_ops = &ffi::platform_profile_ops {
    probe: Some(profile_probe),
    hidden_choices: None,
    profile_get: Some(profile_get),
    profile_set: Some(profile_set),
};
fn sensor(kind: ffi::hwmon_sensor_types, channel: i32) -> Option<u8> {
    match (kind, channel) {
        (ffi::hwmon_temp, 0) => Some(1),
        (ffi::hwmon_temp, 1) => Some(10),
        (ffi::hwmon_temp, 2) => Some(3),
        (ffi::hwmon_fan, 0) => Some(2),
        (ffi::hwmon_fan, 1) => Some(6),
        _ => None,
    }
}
unsafe extern "C" fn hwmon_visible(
    data: *const c_void,
    kind: ffi::hwmon_sensor_types,
    _: u32,
    channel: i32,
) -> u16 {
    let Some(sensor) = sensor(kind, channel) else {
        return 0;
    };
    let supported = unsafe { &*data.cast::<AtomicU64>() }.load(Ordering::Relaxed);
    if supported & (1 << (sensor - 1)) != 0 {
        0o444
    } else {
        0
    }
}
unsafe extern "C" fn hwmon_read(
    _: *mut ffi::device,
    kind: ffi::hwmon_sensor_types,
    _: u32,
    channel: i32,
    value: *mut c_long,
) -> c_int {
    let Some(sensor) = sensor(kind, channel) else {
        return -95;
    };
    match crate::with_state(|state| gaming::temperature(state, sensor)) {
        Ok(reading) => {
            unsafe {
                *value = reading as c_long * if kind == ffi::hwmon_temp { 1000 } else { 1 };
            }
            0
        }
        Err(error) => error,
    }
}
const HWMON_OPS: &'static ffi::hwmon_ops = &ffi::hwmon_ops {
    visible: 0,
    is_visible: Some(hwmon_visible),
    read: Some(hwmon_read),
    read_string: None,
    write: None,
};
static TEMP_CONFIG: [u32; 4] = [
    1 << ffi::hwmon_temp_input,
    1 << ffi::hwmon_temp_input,
    1 << ffi::hwmon_temp_input,
    0,
];
static FAN_CONFIG: [u32; 3] = [1 << ffi::hwmon_fan_input, 1 << ffi::hwmon_fan_input, 0];
const TEMP_CHANNEL: &'static ffi::hwmon_channel_info = &ffi::hwmon_channel_info {
    type_: ffi::hwmon_temp,
    config: TEMP_CONFIG.as_ptr(),
};
const FAN_CHANNEL: &'static ffi::hwmon_channel_info = &ffi::hwmon_channel_info {
    type_: ffi::hwmon_fan,
    config: FAN_CONFIG.as_ptr(),
};
const CHANNELS: &'static [*const ffi::hwmon_channel_info; 3] =
    &[TEMP_CHANNEL, FAN_CHANNEL, ptr::null()];
const HWMON_INFO: &'static ffi::hwmon_chip_info = &ffi::hwmon_chip_info {
    ops: HWMON_OPS,
    info: CHANNELS.as_ptr(),
};
fn register_input(state: &mut State) -> Result<()> {
    if !state.endpoints.has(EVENTS) {
        return Ok(());
    }
    let input = pointer_error(unsafe { ffi::input_allocate_device() })?;
    unsafe {
        (*input).name = c"Predator PHN16-71 hotkeys".as_ptr();
        (*input).phys = c"wmi/input0".as_ptr();
        (*input).id.bustype = ffi::BUS_HOST as u16;
    }
    let result = (|| {
        transport::error(unsafe {
            ffi::sparse_keymap_setup(input, keymap::KEYMAP.as_ptr(), None)
        })?;
        transport::error(unsafe { ffi::input_register_device(input) })
    })();
    if let Err(error) = result {
        unsafe {
            ffi::input_free_device(input);
        }
        return Err(error);
    }
    state.input.store(input, Ordering::Relaxed);
    Ok(())
}
pub(crate) fn hotkey(state: &State, scancode: u32) -> Result<()> {
    let input = state.input();
    if !input.is_null() {
        // SAFETY: the STATE guard pins the input device until unregister drains callbacks.
        unsafe {
            ffi::sparse_keymap_report_event(input, scancode, 1, true);
        }
    }
    Ok(())
}
pub(crate) fn register(state: &mut State) -> Result<()> {
    let parent = unsafe { ptr::addr_of_mut!((*state.platform()).dev) };
    // Firmware can transiently report no choices during initialization. Refresh
    // the capability query on each bounded retry instead of sleeping for seconds
    // under STATE with stale choices. A missing profile interface is an error.
    for attempt in 0..3 {
        if let Ok(raw) = state.endpoints.misc_get(10) {
            let (choices, maximum, previous) = policy::profile_choices(raw);
            if choices != 0 {
                PROFILE_CHOICES.store(choices, Ordering::Relaxed);
                let device = unsafe {
                    ffi::platform_profile_register(
                        parent,
                        c"acer-wmi".as_ptr(),
                        ptr::null_mut(),
                        PROFILE_OPS,
                    )
                };
                if let Ok(device) = pointer_error(device) {
                    state.supported_profiles = raw;
                    state.maximum_profile = maximum;
                    state.previous_profile = previous;
                    state.profile_device.store(device, Ordering::Relaxed);
                    break;
                }
            }
        }
        if attempt < 2 {
            unsafe {
                ffi::msleep(10);
            }
        }
    }
    if state.profile_device().is_null() {
        return Err(-5);
    }
    sysfs::create(state)?;
    if let Ok(result) = state.endpoints.system_info(0) {
        state.supported_sensors = (result >> 24) & 0xffff;
        SUPPORTED_SENSORS.store(state.supported_sensors, Ordering::Relaxed);
        if state.supported_sensors != 0 {
            let hwmon = pointer_error(unsafe {
                ffi::hwmon_device_register_with_info(
                    parent,
                    c"acer".as_ptr(),
                    ptr::addr_of!(SUPPORTED_SENSORS).cast_mut().cast(),
                    HWMON_INFO,
                    ptr::null_mut(),
                )
            })?;
            state.hwmon.store(hwmon, Ordering::Relaxed);
        }
    }
    if let Ok(keyboard) = gaming::keyboard_get(state) {
        state.keyboard = keyboard;
        state.keyboard_valid = true;
    }
    register_input(state)
}
pub(crate) fn unregister(state: &mut State) {
    state.ready = false;
    sysfs::remove(state);
    let input = state.input.swap(ptr::null_mut(), Ordering::Relaxed);
    if !input.is_null() {
        unsafe {
            ffi::input_unregister_device(input);
        }
    }
    let hwmon = state.hwmon.swap(ptr::null_mut(), Ordering::Relaxed);
    if !hwmon.is_null() {
        unsafe {
            ffi::hwmon_device_unregister(hwmon);
        }
    }
    let profile = state
        .profile_device
        .swap(ptr::null_mut(), Ordering::Relaxed);
    if !profile.is_null() {
        unsafe {
            ffi::platform_profile_remove(profile);
        }
    }
}
pub(crate) fn suspend(state: &mut State) -> Result<()> {
    state.suspended = true;
    Ok(())
}
pub(crate) fn resume(state: &mut State) -> Result<()> {
    state.suspended = false;
    gaming::apply_fan_policy(state)?;
    if state.keyboard_valid {
        gaming::keyboard_set(state, state.keyboard)?;
    }
    Ok(())
}
