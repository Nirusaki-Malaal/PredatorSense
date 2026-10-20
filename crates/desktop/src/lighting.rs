//! Small, allocation-free colour and animation model for the local preview.
#![forbid(unsafe_code)]
pub const EFFECTS: [&str; 8] = [
    "Static",
    "Breathing",
    "Neon spectrum",
    "Rainbow wave",
    "Shifting",
    "Zoom",
    "Meteor",
    "Twinkling",
];
#[derive(Clone, Copy)]
pub struct Lighting {
    pub zones: bool,
    pub effect: u32,
    pub speed: f64,
    pub brightness: f64,
    pub reverse: bool,
    pub colors: [[f64; 3]; 4],
    pub selected: usize,
    pub playing: bool,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            zones: false,
            effect: 0,
            speed: 3.0,
            brightness: 1.0,
            reverse: false,
            colors: [[0.27, 0.87, 0.8]; 4],
            selected: 0,
            playing: true,
        }
    }
}
pub fn parse_color(value: &str) -> Option<[f64; 3]> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.is_ascii() {
        return None;
    }
    let number = u32::from_str_radix(value, 16).ok()?;
    Some([
        ((number >> 16) & 255) as f64 / 255.0,
        ((number >> 8) & 255) as f64 / 255.0,
        (number & 255) as f64 / 255.0,
    ])
}
pub fn hex(color: [f64; 3]) -> String {
    format!(
        "{:02x}{:02x}{:02x}",
        channel(color[0]),
        channel(color[1]),
        channel(color[2])
    )
}
pub fn channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn spectrum(hue: f64) -> [f64; 3] {
    let h = hue.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h.rem_euclid(2.0) - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}
impl Lighting {
    pub fn animated(&self) -> bool {
        self.playing && !self.zones && self.effect != 0
    }
    pub fn color(&self, x: f64, y: f64, id: usize, seconds: f64) -> [f64; 3] {
        let zone = ((x / 19.5 * 4.0) as usize).min(3);
        let time = seconds * (0.3 + self.speed * 0.16);
        let position = if self.reverse {
            1.0 - x / 19.5
        } else {
            x / 19.5
        };
        let mut color = if self.zones {
            self.colors[zone]
        } else {
            self.colors[0]
        };
        let mut level = self.brightness;
        if !self.zones {
            match self.effect {
                1 => level *= 0.12 + 0.88 * (time * 2.0).sin().powi(2),
                2 => color = spectrum(time * 0.18),
                3 => color = spectrum(position - time * 0.24),
                4 => {
                    let wave = (position * 7.0 - time * 3.0).sin() * 0.5 + 0.5;
                    color = color.map(|component| component * (0.15 + 0.85 * wave));
                }
                5 => {
                    let radius = ((position - 0.5).powi(2) + ((y - 3.0) / 12.0).powi(2)).sqrt();
                    level *= 0.08 + 0.92 * (radius * 15.0 - time * 4.0).cos().max(0.0).powi(3);
                }
                6 => {
                    let distance = (time * 0.35 - position).rem_euclid(1.3);
                    level *= if distance < 0.32 {
                        (1.0 - distance / 0.32).powi(2)
                    } else {
                        0.04
                    };
                }
                7 => {
                    level *= 0.06 + 0.94 * (time * 2.8 + id as f64 * 2.399).sin().max(0.0).powi(12)
                }
                _ => {}
            }
        }
        color.map(|component| (component * level).clamp(0.0, 1.0))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_parsing_is_strict_and_round_trips() {
        for color in ["000000", "ff00aa", "abcdef", "FFFFFF"] {
            assert_eq!(hex(parse_color(color).unwrap()), color.to_lowercase());
        }
        assert_eq!(parse_color("#123456"), parse_color("123456"));
        for value in ["abc", "gg0000", "1234567", "１２３４５６"] {
            assert!(parse_color(value).is_none());
        }
    }
    #[test]
    fn effects_change_over_time_and_respect_brightness() {
        let mut state = Lighting::default();
        for effect in 1..8 {
            state.effect = effect;
            assert!(
                (0..20).any(|key| state.color(key as f64, 2.0, key, 0.0)
                    != state.color(key as f64, 2.0, key, 0.7)),
                "effect {effect}"
            );
            state.brightness = 0.0;
            assert_eq!(state.color(9.0, 2.0, 4, 1.0), [0.0; 3]);
            state.brightness = 1.0;
        }
        state.zones = true;
        assert_eq!(state.color(9.0, 2.0, 4, 0.0), state.color(9.0, 2.0, 4, 1.0));
    }
}
