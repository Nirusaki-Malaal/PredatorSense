// SPDX-License-Identifier: GPL-2.0-or-later
//! PHN16-71 predator_sense and four_zoned_kb sysfs interfaces.
use crate::{
    ffi, gaming,
    policy::{self, CurvePoint, CurveProfile, FanMode},
    protocol,
    state::State,
    transport::{error, Result, GAMING},
};
use core::{
    ffi::{c_char, CStr},
    fmt::{self, Write},
    ptr, slice,
};
const fn attribute(name: &'static CStr) -> ffi::device_attribute {
    let mut value: ffi::device_attribute = unsafe { core::mem::zeroed() };
    value.attr.name = name.as_ptr();
    value.attr.mode = 0o644;
    value.__bindgen_anon_1.show = Some(show);
    value.__bindgen_anon_2.store = Some(store);
    value
}
const BOOT: &'static ffi::device_attribute = &attribute(c"boot_animation_sound");
const TIMEOUT: &'static ffi::device_attribute = &attribute(c"backlight_timeout");
const USB: &'static ffi::device_attribute = &attribute(c"usb_charging");
const LIMIT: &'static ffi::device_attribute = &attribute(c"battery_limiter");
const CALIBRATION: &'static ffi::device_attribute = &attribute(c"battery_calibration");
const FAN: &'static ffi::device_attribute = &attribute(c"fan_speed");
const FAN_MODE: &'static ffi::device_attribute = &attribute(c"thermal_fan_mode");
const CURVE_PROFILE: &'static ffi::device_attribute = &attribute(c"fan_curve_profile");
const CURVE_POINTS: &'static ffi::device_attribute = &attribute(c"fan_curve_points");
const LCD: &'static ffi::device_attribute = &attribute(c"lcd_override");
const EFFECT: &'static ffi::device_attribute = &attribute(c"four_zone_mode");
const ZONES: &'static ffi::device_attribute = &attribute(c"per_zone_mode");
const PREDATOR_ATTRIBUTES: &'static [*mut ffi::attribute; 11] = &[
    ptr::addr_of!(BOOT.attr).cast_mut(),
    ptr::addr_of!(TIMEOUT.attr).cast_mut(),
    ptr::addr_of!(USB.attr).cast_mut(),
    ptr::addr_of!(LIMIT.attr).cast_mut(),
    ptr::addr_of!(CALIBRATION.attr).cast_mut(),
    ptr::addr_of!(FAN.attr).cast_mut(),
    ptr::addr_of!(FAN_MODE.attr).cast_mut(),
    ptr::addr_of!(CURVE_PROFILE.attr).cast_mut(),
    ptr::addr_of!(CURVE_POINTS.attr).cast_mut(),
    ptr::addr_of!(LCD.attr).cast_mut(),
    ptr::null_mut(),
];
const KEYBOARD_ATTRIBUTES: &'static [*mut ffi::attribute; 3] = &[
    ptr::addr_of!(EFFECT.attr).cast_mut(),
    ptr::addr_of!(ZONES.attr).cast_mut(),
    ptr::null_mut(),
];
const fn group(name: &'static CStr, attrs: *mut *mut ffi::attribute) -> ffi::attribute_group {
    let mut value: ffi::attribute_group = unsafe { core::mem::zeroed() };
    value.name = name.as_ptr();
    value.__bindgen_anon_2.attrs = attrs;
    value
}
const PREDATOR: &'static ffi::attribute_group =
    &group(c"predator_sense", PREDATOR_ATTRIBUTES.as_ptr().cast_mut());
const KEYBOARD: &'static ffi::attribute_group =
    &group(c"four_zoned_kb", KEYBOARD_ATTRIBUTES.as_ptr().cast_mut());
const GROUPS: [&ffi::attribute_group; 2] = [PREDATOR, KEYBOARD];
pub(crate) fn create(state: &mut State) -> Result<()> {
    for (index, group) in GROUPS.iter().enumerate() {
        let result = unsafe {
            ffi::sysfs_create_group(ptr::addr_of_mut!((*state.platform()).dev.kobj), *group)
        };
        error(result)?;
        state.groups |= 1 << index;
    }
    Ok(())
}
pub(crate) fn remove(state: &mut State) {
    for (index, group) in GROUPS.iter().enumerate().rev() {
        if state.groups & (1 << index) != 0 {
            unsafe {
                ffi::sysfs_remove_group(ptr::addr_of_mut!((*state.platform()).dev.kobj), *group);
            }
        }
    }
    state.groups = 0;
}
struct Output {
    bytes: [u8; 4096],
    length: usize,
}
impl Output {
    fn new() -> Self {
        Self {
            bytes: [0; 4096],
            length: 0,
        }
    }
}
impl Write for Output {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if end >= self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.length..end].copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}
fn read(state: &State, name: &[u8], out: &mut Output) -> Result<()> {
    let result = match name {
        b"boot_animation_sound" => writeln!(
            out,
            "{}",
            (state.endpoints.number64(GAMING, 23, 6)? == 0x100) as u8
        ),
        b"backlight_timeout" => writeln!(out, "{}", gaming::timeout_get(state)? as u8),
        b"usb_charging" => writeln!(out, "{}", gaming::usb_get(state)?),
        b"battery_limiter" => writeln!(out, "{}", gaming::battery_get(state, false)? as u8),
        b"battery_calibration" => writeln!(out, "{}", gaming::battery_get(state, true)? as u8),
        b"fan_speed" => writeln!(out, "{},{}", state.cpu_speed, state.gpu_speed),
        b"thermal_fan_mode" => writeln!(
            out,
            "{}",
            match state.fan_mode {
                FanMode::Auto => "auto",
                FanMode::Fixed => "fixed",
                FanMode::Curve => "curve",
            }
        ),
        b"fan_curve_profile" => writeln!(
            out,
            "{}",
            match state.curve_profile {
                CurveProfile::Conservative => "conservative",
                CurveProfile::Aggressive => "aggressive",
                CurveProfile::Custom => "custom",
            }
        ),
        b"fan_curve_points" => {
            for (index, point) in state.curve_points[..state.point_count].iter().enumerate() {
                if index != 0 {
                    write!(out, ",").map_err(|_| -7)?;
                }
                write!(out, "{}:{}:{}", point.temperature, point.cpu, point.gpu).map_err(|_| -7)?;
            }
            writeln!(out)
        }
        b"lcd_override" => {
            let result = state.endpoints.number64(GAMING, 3, 0)?;
            let enabled = match result {
                0x1000001000000 => 1,
                0x1000000 => 0,
                _ => -1,
            };
            writeln!(out, "{enabled}")
        }
        b"four_zone_mode" => {
            let reply = state
                .endpoints
                .invoke(GAMING, 21, &1u64.to_le_bytes(), 16)?;
            let bytes = reply.bytes();
            if bytes[0] != 0 {
                return Err(-5);
            }
            writeln!(
                out,
                "{},{},{},{},{},{},{}",
                bytes[1], bytes[2], bytes[3], bytes[5], bytes[6], bytes[7], bytes[8]
            )
        }
        b"per_zone_mode" => {
            let keyboard = gaming::keyboard_get(state)?;
            writeln!(
                out,
                "{:06x},{:06x},{:06x},{:06x},{}",
                keyboard.colors[0],
                keyboard.colors[1],
                keyboard.colors[2],
                keyboard.colors[3],
                keyboard.zone_brightness
            )
        }
        _ => return Err(-95),
    };
    result.map_err(|_| -7)
}
fn boolean(value: &[u8]) -> Result<bool> {
    match value {
        b"0" => Ok(false),
        b"1" => Ok(true),
        _ => Err(-22),
    }
}
fn write(state: &mut State, name: &[u8], value: &[u8]) -> Result<()> {
    match name {
        b"boot_animation_sound" => {
            state
                .endpoints
                .number64(GAMING, 22, if boolean(value)? { 0x106 } else { 6 })?;
        }
        b"backlight_timeout" => {
            gaming::timeout_set(state, boolean(value)?)?;
        }
        b"usb_charging" => gaming::usb_set(state, protocol::parse(value).ok_or(-22)?)?,
        b"battery_limiter" => gaming::battery_set(state, false, boolean(value)?)?,
        b"battery_calibration" => gaming::battery_set(state, true, boolean(value)?)?,
        b"fan_speed" => {
            let speeds = protocol::numbers::<2>(value).ok_or(-22)?;
            if speeds.iter().any(|v| *v > 100) {
                return Err(-22);
            }
            let mode = if speeds == [0, 0] {
                FanMode::Auto
            } else {
                FanMode::Fixed
            };
            gaming::write_fans(state, speeds[0] as i32, speeds[1] as i32)?;
            state.fan_mode = mode;
            state.reset_hysteresis();
        }
        b"thermal_fan_mode" => {
            let mode = match value {
                b"auto" => FanMode::Auto,
                b"fixed" => FanMode::Fixed,
                b"curve" => FanMode::Curve,
                _ => return Err(-22),
            };
            let previous = state.fan_mode;
            state.fan_mode = mode;
            if let Err(error) = gaming::apply_fan_policy(state) {
                state.fan_mode = previous;
                return Err(error);
            }
        }
        b"fan_curve_profile" => {
            state.curve_profile = match value {
                b"conservative" => CurveProfile::Conservative,
                b"aggressive" => CurveProfile::Aggressive,
                b"custom" => CurveProfile::Custom,
                _ => return Err(-22),
            };
            if state.fan_mode == FanMode::Curve {
                state.reset_hysteresis();
            }
        }
        b"fan_curve_points" => {
            let mut points = state.curve_points;
            let mut count = 0;
            for token in value.split(|v| *v == b',') {
                if count == 8 {
                    return Err(-22);
                }
                let mut fields = token.split(|v| *v == b':');
                let temperature = protocol::parse(fields.next().ok_or(-22)?).ok_or(-22)?;
                let cpu = protocol::parse(fields.next().ok_or(-22)?).ok_or(-22)?;
                let gpu = protocol::parse(fields.next().ok_or(-22)?).ok_or(-22)?;
                if fields.next().is_some() {
                    return Err(-22);
                }
                points[count] = CurvePoint {
                    temperature: temperature as i32,
                    cpu: cpu as i32,
                    gpu: gpu as i32,
                };
                count += 1;
            }
            if !policy::valid_points(&points[..count]) {
                return Err(-22);
            }
            state.curve_points = points;
            state.point_count = count;
            state.reset_hysteresis();
        }
        b"lcd_override" => {
            state.endpoints.number64(
                GAMING,
                1,
                if boolean(value)? {
                    0x1000000000010
                } else {
                    0x10
                },
            )?;
        }
        b"four_zone_mode" => {
            let bytes = protocol::effect(value).ok_or(-22)?;
            let mut keyboard = state.keyboard;
            keyboard.per_zone = false;
            keyboard.mode = bytes[0];
            keyboard.speed = bytes[1];
            keyboard.brightness = bytes[2];
            keyboard.direction = bytes[4];
            keyboard.red = bytes[5];
            keyboard.green = bytes[6];
            keyboard.blue = bytes[7];
            gaming::keyboard_set(state, keyboard)?;
        }
        b"per_zone_mode" => {
            let (colors, brightness) = protocol::zones(value).ok_or(-22)?;
            let mut keyboard = state.keyboard;
            keyboard.per_zone = true;
            keyboard.zone_brightness = brightness as i32;
            for (index, rgb) in colors.iter().enumerate() {
                keyboard.colors[index] =
                    ((rgb[0] as u64) << 16) | ((rgb[1] as u64) << 8) | rgb[2] as u64;
            }
            gaming::keyboard_set(state, keyboard)?;
        }
        _ => return Err(-95),
    }
    Ok(())
}
unsafe extern "C" fn show(
    _: *mut ffi::device,
    attribute: *mut ffi::device_attribute,
    buffer: *mut c_char,
) -> isize {
    let name = unsafe { CStr::from_ptr((*attribute).attr.name) }.to_bytes();
    let mut output = Output::new();
    match crate::with_state(|state| read(state, name, &mut output)) {
        Ok(()) => {
            unsafe {
                ptr::copy_nonoverlapping(output.bytes.as_ptr(), buffer.cast(), output.length);
                *buffer.add(output.length) = 0;
            }
            output.length as isize
        }
        Err(error) => error as isize,
    }
}
unsafe extern "C" fn store(
    _: *mut ffi::device,
    attribute: *mut ffi::device_attribute,
    buffer: *const c_char,
    length: usize,
) -> isize {
    if length == 0 || length > 256 {
        return -22;
    }
    let name = unsafe { CStr::from_ptr((*attribute).attr.name) }.to_bytes();
    let value = unsafe { slice::from_raw_parts(buffer.cast(), length) };
    let value = value.strip_suffix(b"\n").unwrap_or(value);
    match crate::with_state(|state| write(state, name, value)) {
        Ok(()) => length as isize,
        Err(error) => error as isize,
    }
}
