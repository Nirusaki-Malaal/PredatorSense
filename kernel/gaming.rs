// SPDX-License-Identifier: GPL-2.0-or-later
//! Gaming controls and their firmware payloads, preserving the original ABI.
use crate::{
    ffi,
    policy::{self, CurveProfile, FanMode},
    state::{KeyboardState, State},
    transport::{Result, ACTION, BATTERY, GAMING},
};
pub(crate) fn on_ac(state: &State) -> Result<bool> {
    Ok(state.endpoints.number64(GAMING, 5, 2)? != 0)
}
pub(crate) fn set_fan_behavior(state: &State, command: u64) -> Result<()> {
    state
        .endpoints
        .procedure(GAMING, 14, &command.to_le_bytes())?;
    Ok(())
}
pub(crate) fn write_fans(state: &mut State, cpu: i32, gpu: i32) -> Result<()> {
    if !(0..=100).contains(&cpu) || !(0..=100).contains(&gpu) {
        return Err(-22);
    }
    let set_speed = |speed: i32, index: u64| -> Result<()> {
        let payload = ((speed as u64 * 25600 / 100) & 0xff00) + index;
        state
            .endpoints
            .procedure(GAMING, 16, &payload.to_le_bytes())?;
        Ok(())
    };
    match (cpu, gpu) {
        (0, 0) => set_fan_behavior(state, 0x410009)?,
        (100, 100) => set_fan_behavior(state, 0x820009)?,
        (0, gpu) => {
            set_fan_behavior(state, 0x10001)?;
            set_fan_behavior(state, 0xc00008)?;
            set_speed(gpu, 4)?;
        }
        (cpu, 0) => {
            set_fan_behavior(state, 0x400008)?;
            set_fan_behavior(state, 0x30001)?;
            set_speed(cpu, 1)?;
        }
        (cpu, gpu) => {
            set_fan_behavior(state, 0xc30009)?;
            set_speed(cpu, 1)?;
            set_speed(gpu, 4)?;
        }
    }
    state.cpu_speed = cpu;
    state.gpu_speed = gpu;
    Ok(())
}
pub(crate) fn apply_fan_policy(state: &mut State) -> Result<()> {
    state.reset_hysteresis();
    // A power profile selects firmware power limits, not a different fan policy.
    // Keep the user's automatic, fixed or curve selection in Turbo as well.
    match state.fan_mode {
        FanMode::Auto => write_fans(state, 0, 0),
        FanMode::Fixed => write_fans(state, state.cpu_speed, state.gpu_speed),
        FanMode::Curve => curve_tick(state),
    }
}
pub(crate) fn temperature(state: &State, sensor: u8) -> Result<i32> {
    let result = state.endpoints.system_info(1 | ((sensor as u32) << 8))?;
    Ok(((result >> 8) & 0xffff) as i32)
}
pub(crate) fn curve_tick(state: &mut State) -> Result<()> {
    if state.suspended || state.fan_mode != FanMode::Curve {
        return Ok(());
    }
    let (cpu, gpu) = match (temperature(state, 1), temperature(state, 10)) {
        (Ok(cpu), Ok(gpu)) => (cpu, gpu),
        (Ok(temp), Err(_)) | (Err(_), Ok(temp)) => (temp, temp),
        (Err(_), Err(_)) => return write_fans(state, 100, 100),
    };
    let points = &state.curve_points[..state.point_count];
    let cpu_target = policy::fan_speed(cpu, true, state.curve_profile, points);
    let gpu_target = policy::fan_speed(gpu, false, state.curve_profile, points);
    let previous_cpu = state.cpu_hysteresis;
    let previous_gpu = state.gpu_hysteresis;
    let cpu_speed = state.cpu_hysteresis.apply(cpu, cpu_target);
    let gpu_speed = state.gpu_hysteresis.apply(gpu, gpu_target);
    if let Err(error) = write_fans(state, cpu_speed, gpu_speed) {
        // A failed firmware write must not advance the smoothing policy.
        state.cpu_hysteresis = previous_cpu;
        state.gpu_hysteresis = previous_gpu;
        return Err(error);
    }
    Ok(())
}
pub(crate) fn set_profile(state: &mut State, profile: u8) -> Result<()> {
    if !matches!(profile, 0 | 1 | 4 | 5 | 6) {
        return Err(-22);
    }
    if !on_ac(state)? && matches!(profile, 0 | 4 | 5) {
        return Err(-95);
    }
    state.endpoints.misc_set(11, profile)?;
    if profile != state.maximum_profile {
        state.previous_profile = profile;
    }
    apply_fan_policy(state)
}
pub(crate) fn cycle_profile(state: &mut State) -> Result<()> {
    let current = state.endpoints.misc_get(11)?;
    let next = policy::next_profile(
        current,
        on_ac(state)?,
        crate::parameters::cycle_profiles(),
        state.maximum_profile,
        state.previous_profile,
    )
    .ok_or(-95)?;
    // Skip unsupported choices rather than writing a profile firmware does not
    // advertise. This fixes mode buttons on firmware with missing profiles.
    let next = if state.supported_profiles & (1 << next) != 0 {
        next
    } else {
        let mut candidate = next;
        let mut selected = None;
        for _ in 0..5 {
            candidate = match candidate {
                6 => 0,
                0 => 1,
                1 => 4,
                4 => 5,
                _ => 6,
            };
            if state.supported_profiles & (1 << candidate) != 0
                && (on_ac(state)? || matches!(candidate, 1 | 6))
            {
                selected = Some(candidate);
                break;
            }
        }
        selected.ok_or(-95)?
    };
    set_profile(state, next)?;
    kernel::pr_info!("Mode key changed firmware profile to {}\n", next);
    if !state.profile_device().is_null() {
        unsafe {
            ffi::platform_profile_notify(state.profile_device());
        }
    }
    Ok(())
}
pub(crate) fn battery_get(state: &State, calibration: bool) -> Result<bool> {
    let reply = state.endpoints.invoke(BATTERY, 20, &[1, 1, 0, 0], 8)?;
    Ok(reply.bytes()[if calibration { 4 } else { 3 }] != 0)
}
pub(crate) fn battery_set(state: &State, calibration: bool, enabled: bool) -> Result<()> {
    let reply = state.endpoints.invoke(
        BATTERY,
        21,
        &[
            1,
            if calibration { 2 } else { 1 },
            enabled as u8,
            0,
            0,
            0,
            0,
            0,
        ],
        2,
    )?;
    if reply.bytes()[0] != 0 || reply.bytes()[1] != 0 {
        return Err(-5);
    }
    Ok(())
}
pub(crate) fn timeout_get(state: &State) -> Result<bool> {
    match state.endpoints.number64(ACTION, 2, 0x88401)? {
        0x1e0000080000 => Ok(true),
        0x80000 => Ok(false),
        _ => Err(-34),
    }
}
pub(crate) fn timeout_set(state: &State, enabled: bool) -> Result<()> {
    state
        .endpoints
        .number64(ACTION, 1, if enabled { 0x1e0000088402 } else { 0x88402 })?;
    Ok(())
}
pub(crate) fn usb_get(state: &State) -> Result<u32> {
    match state.endpoints.number64(ACTION, 2, 4)? {
        663296 => Ok(0),
        659200 => Ok(10),
        1314560 => Ok(20),
        1969920 => Ok(30),
        _ => Err(-34),
    }
}
pub(crate) fn usb_set(state: &State, value: u32) -> Result<()> {
    let input = match value {
        0 => 663300,
        10 => 659204,
        20 => 1314564,
        30 => 1969924,
        _ => return Err(-22),
    };
    state.endpoints.number64(ACTION, 1, input)?;
    Ok(())
}
pub(crate) fn keyboard_effect_get(state: &State) -> Result<KeyboardState> {
    let reply = state
        .endpoints
        .invoke(GAMING, 21, &1u64.to_le_bytes(), 16)?;
    let bytes = reply.bytes();
    if bytes[0] != 0 {
        return Err(-5);
    }
    let mut keyboard = state.keyboard;
    keyboard.mode = bytes[1];
    keyboard.speed = bytes[2];
    keyboard.brightness = bytes[3];
    keyboard.direction = bytes[5];
    keyboard.red = bytes[6];
    keyboard.green = bytes[7];
    keyboard.blue = bytes[8];
    keyboard.zone_brightness = bytes[3] as i32;
    if !keyboard.valid() {
        return Err(-5);
    }
    Ok(keyboard)
}
pub(crate) fn keyboard_get(state: &State) -> Result<KeyboardState> {
    let mut keyboard = keyboard_effect_get(state)?;
    for (index, zone) in [1u64, 2, 4, 8].iter().enumerate() {
        let color = state.endpoints.number64(GAMING, 7, *zone)?;
        keyboard.colors[index] = (color.to_be() >> 32) & 0xffffff;
    }
    if !keyboard.valid() {
        return Err(-5);
    }
    Ok(keyboard)
}
fn effect_write(state: &State, keyboard: &KeyboardState) -> Result<()> {
    let bytes = [
        keyboard.mode,
        keyboard.speed,
        keyboard.brightness,
        0,
        keyboard.direction,
        keyboard.red,
        keyboard.green,
        keyboard.blue,
        3,
        1,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let reply = state.endpoints.invoke(GAMING, 20, &bytes, 4)?;
    if reply.number()? != 0 {
        return Err(-5);
    }
    Ok(())
}
pub(crate) fn keyboard_set(state: &mut State, mut keyboard: KeyboardState) -> Result<()> {
    if !keyboard.valid() {
        return Err(-22);
    }
    if keyboard.per_zone {
        let mut static_mode = keyboard;
        static_mode.mode = 0;
        static_mode.speed = 0;
        static_mode.direction = 0;
        static_mode.red = 0;
        static_mode.green = 0;
        static_mode.blue = 0;
        static_mode.brightness = keyboard.zone_brightness as u8;
        effect_write(state, &static_mode)?;
        for (index, color) in keyboard.colors.iter().enumerate() {
            let bytes = [
                1u8 << index,
                (color >> 16) as u8,
                (color >> 8) as u8,
                *color as u8,
            ];
            state.endpoints.procedure(GAMING, 6, &bytes)?;
        }
    } else {
        match keyboard.mode {
            0 | 1 => {
                keyboard.speed = 0;
                keyboard.direction = 0;
            }
            2 => {
                keyboard.red = 0;
                keyboard.green = 0;
                keyboard.blue = 0;
                keyboard.direction = 0;
            }
            3 => {
                keyboard.red = 0;
                keyboard.green = 0;
                keyboard.blue = 0;
            }
            4 => {}
            _ => keyboard.direction = 0,
        }
        effect_write(state, &keyboard)?;
    }
    // Cache the successful request, not a later firmware read. Readback is
    // unreliable while effects run and used to overwrite the saved mode.
    state.keyboard = keyboard;
    state.keyboard_valid = true;
    Ok(())
}
pub(crate) fn save_power_snapshot(state: &mut State, index: usize) -> Result<()> {
    let mut snapshot = state.power_states[index];
    snapshot.thermal_profile = state.endpoints.misc_get(11)? as i32;
    if let Ok(timeout) = timeout_get(state) {
        snapshot.backlight_timeout = timeout as i32;
    }
    snapshot.cpu_fan_speed = state.cpu_speed;
    snapshot.gpu_fan_speed = state.gpu_speed;
    snapshot.fan_mode = state.fan_mode as i32;
    snapshot.fan_curve_profile = state.curve_profile as i32;
    snapshot.custom_fan_curve_points = state.point_count as i32;
    snapshot.custom_fan_curve = state.curve_points;
    state.power_states[index] = snapshot;
    Ok(())
}
pub(crate) fn restore_power_snapshot(state: &mut State, index: usize) -> Result<()> {
    let snapshot = state.power_states[index];
    if !snapshot.valid() {
        return Err(-22);
    }
    state
        .endpoints
        .misc_set(11, snapshot.thermal_profile as u8)?;
    let _ = timeout_set(state, snapshot.backlight_timeout != 0);
    state.cpu_speed = snapshot.cpu_fan_speed;
    state.gpu_speed = snapshot.gpu_fan_speed;
    state.fan_mode = match snapshot.fan_mode {
        1 => FanMode::Fixed,
        2 => FanMode::Curve,
        _ => FanMode::Auto,
    };
    state.curve_profile = match snapshot.fan_curve_profile {
        1 => CurveProfile::Aggressive,
        2 => CurveProfile::Custom,
        _ => CurveProfile::Conservative,
    };
    state.point_count = snapshot.custom_fan_curve_points as usize;
    state.curve_points = snapshot.custom_fan_curve;
    apply_fan_policy(state)
}
