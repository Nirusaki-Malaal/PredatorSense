#![forbid(unsafe_code)]
// SPDX-License-Identifier: GPL-2.0-or-later
//! Thermal policy translated from predatrix_sense.c without changing thresholds.
pub(crate) const MAX_POINTS: usize = 8;
pub(crate) const CURVE_INTERVAL_SECONDS: u64 = 5;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub(crate) struct CurvePoint {
    pub(crate) temperature: i32,
    pub(crate) cpu: i32,
    pub(crate) gpu: i32,
}
pub(crate) const DEFAULT_POINTS: [CurvePoint; MAX_POINTS] = [
    CurvePoint {
        temperature: 55,
        cpu: 0,
        gpu: 0,
    },
    CurvePoint {
        temperature: 65,
        cpu: 35,
        gpu: 35,
    },
    CurvePoint {
        temperature: 75,
        cpu: 55,
        gpu: 60,
    },
    CurvePoint {
        temperature: 85,
        cpu: 80,
        gpu: 85,
    },
    CurvePoint {
        temperature: 90,
        cpu: 100,
        gpu: 100,
    },
    CurvePoint {
        temperature: 0,
        cpu: 0,
        gpu: 0,
    },
    CurvePoint {
        temperature: 0,
        cpu: 0,
        gpu: 0,
    },
    CurvePoint {
        temperature: 0,
        cpu: 0,
        gpu: 0,
    },
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FanMode {
    Auto = 0,
    Fixed = 1,
    Curve = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CurveProfile {
    Conservative = 0,
    Aggressive = 1,
    Custom = 2,
}
pub(crate) fn valid_points(points: &[CurvePoint]) -> bool {
    (2..=MAX_POINTS).contains(&points.len())
        && points.iter().all(|point| {
            (30..=100).contains(&point.temperature)
                && (0..=100).contains(&point.cpu)
                && (0..=100).contains(&point.gpu)
        })
        && points
            .windows(2)
            .all(|pair| pair[0].temperature < pair[1].temperature)
}
pub(crate) fn custom_speed(temperature: i32, cpu: bool, points: &[CurvePoint]) -> i32 {
    let speed = |point: &CurvePoint| if cpu { point.cpu } else { point.gpu };
    if temperature <= points[0].temperature {
        return speed(&points[0]);
    }
    for pair in points.windows(2) {
        if temperature <= pair[1].temperature {
            let before = speed(&pair[0]);
            let after = speed(&pair[1]);
            return before
                + (after - before) * (temperature - pair[0].temperature)
                    / (pair[1].temperature - pair[0].temperature);
        }
    }
    speed(points.last().unwrap())
}
pub(crate) fn fan_speed(
    temperature: i32,
    cpu: bool,
    profile: CurveProfile,
    points: &[CurvePoint],
) -> i32 {
    match profile {
        CurveProfile::Custom => custom_speed(temperature, cpu, points),
        CurveProfile::Aggressive => match temperature {
            ..55 => 0,
            55..65 => 40,
            65..75 => 60,
            75..85 => 80,
            _ => 100,
        },
        CurveProfile::Conservative => match temperature {
            ..60 => 0,
            60..65 => 35,
            65..75 => 50,
            75..80 => 65,
            80..90 => 80,
            _ => 100,
        },
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hysteresis {
    last_temperature: i32,
    last_speed: i32,
}
impl Hysteresis {
    pub(crate) const fn new() -> Self {
        Self {
            last_temperature: i32::MIN,
            last_speed: -1,
        }
    }
    pub(crate) fn apply(&mut self, temperature: i32, target: i32) -> i32 {
        let speed = if self.last_speed < 0 || temperature >= 90 || target >= self.last_speed {
            target
        } else if self.last_temperature != i32::MIN && temperature > self.last_temperature - 3 {
            self.last_speed
        } else {
            target.max(self.last_speed - 15)
        };
        self.last_temperature = temperature;
        self.last_speed = speed;
        speed
    }
}
pub(crate) fn next_profile(
    current: u8,
    on_ac: bool,
    cycle: bool,
    maximum: u8,
    previous: u8,
) -> Option<u8> {
    if !on_ac {
        return Some(if current == 6 { 1 } else { 6 });
    }
    match current {
        5 => Some(if cycle { 0 } else { previous }),
        4 => Some(if maximum == current {
            previous
        } else {
            maximum
        }),
        1 => Some(if cycle { 4 } else { maximum }),
        0 => Some(if cycle { 1 } else { maximum }),
        6 => Some(if cycle { 0 } else { maximum }),
        _ => None,
    }
}
pub(crate) fn profile_choices(raw: u8) -> (u64, u8, u8) {
    let mut choices = 0;
    let mut maximum = 6;
    let mut previous = None;
    for (firmware, platform) in [(6, 0), (0, 2), (1, 3), (4, 4), (5, 5)] {
        if raw & (1 << firmware) != 0 {
            choices |= 1 << platform;
            maximum = firmware;
            if firmware != 4 && firmware != 5 || previous.is_none() {
                previous = Some(firmware);
            }
        }
    }
    (choices, maximum, previous.unwrap_or(6))
}
