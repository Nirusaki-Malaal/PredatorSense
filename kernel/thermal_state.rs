// SPDX-License-Identifier: GPL-2.0-or-later
//! In-memory snapshots for AC and battery. Persistence belongs to the daemon.
#![forbid(unsafe_code)]
use crate::policy::{CurvePoint, DEFAULT_POINTS};
#[derive(Clone, Copy)]
pub(crate) struct ThermalState {
    pub(crate) cpu_fan_speed: i32,
    pub(crate) gpu_fan_speed: i32,
    pub(crate) thermal_profile: i32,
    pub(crate) backlight_timeout: i32,
    pub(crate) fan_mode: i32,
    pub(crate) fan_curve_profile: i32,
    pub(crate) custom_fan_curve_points: i32,
    pub(crate) custom_fan_curve: [CurvePoint; 8],
}
impl ThermalState {
    pub(crate) const fn defaults(on_ac: bool) -> Self {
        Self {
            cpu_fan_speed: 0,
            gpu_fan_speed: 0,
            thermal_profile: if on_ac { 1 } else { 6 },
            backlight_timeout: 1,
            fan_mode: 0,
            fan_curve_profile: 0,
            custom_fan_curve_points: 5,
            custom_fan_curve: DEFAULT_POINTS,
        }
    }
    pub(crate) fn valid(&self) -> bool {
        (0..=100).contains(&self.cpu_fan_speed)
            && (0..=100).contains(&self.gpu_fan_speed)
            && matches!(self.thermal_profile, 0 | 1 | 4 | 5 | 6)
            && matches!(self.backlight_timeout, 0 | 1)
            && (0..=2).contains(&self.fan_mode)
            && (0..=2).contains(&self.fan_curve_profile)
            && (2..=8).contains(&self.custom_fan_curve_points)
            && crate::policy::valid_points(
                &self.custom_fan_curve[..self.custom_fan_curve_points as usize],
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac_and_battery_defaults_are_valid() {
        assert!(ThermalState::defaults(true).valid());
        assert!(ThermalState::defaults(false).valid());
    }
    #[test]
    fn invalid_curve_cannot_be_restored() {
        let mut state = ThermalState::defaults(true);
        state.custom_fan_curve_points = 9;
        assert!(!state.valid());
    }
}
