#![forbid(unsafe_code)]
//! Shared, bounded protocol and validated hardware settings.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};
pub const MAX_FRAME: usize = 8192;
pub const SOCKET: &str = "/run/predator-sense/control.sock";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status,
    Refresh,
    Set { feature: String, value: String },
    Cycle,
    Turbo,
    Restore,
    ConfigureCurve { profile: String, points: String },
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Status {
    pub model: String,
    pub backend: String,
    pub values: BTreeMap<String, String>,
    pub supported: Vec<String>,
    pub choices: Vec<String>,
    pub saved: BTreeMap<String, String>,
    pub errors: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub error: Option<String>,
    pub status: Option<Status>,
}
impl Response {
    pub fn error(e: impl ToString) -> Self {
        Self {
            ok: false,
            error: Some(e.to_string()),
            status: None,
        }
    }
    pub fn success(status: Status) -> Self {
        Self {
            ok: true,
            error: None,
            status: Some(status),
        }
    }
}
pub fn send<T: Serialize>(w: &mut impl Write, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME {
        return Err(io::Error::other("frame too large"));
    }
    w.write_all(&(bytes.len() as u32).to_be_bytes())?;
    w.write_all(&bytes)
}
pub fn receive<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> io::Result<T> {
    let mut length = [0; 4];
    r.read_exact(&mut length)?;
    let n = u32::from_be_bytes(length) as usize;
    if n == 0 || n > MAX_FRAME {
        return Err(io::Error::other("invalid frame length"));
    }
    let mut bytes = vec![0; n];
    r.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
pub fn socket_path() -> std::path::PathBuf {
    std::env::var_os("PREDATOR_SENSE_SOCKET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| SOCKET.into())
}
/// Read one frame within an absolute deadline, including clients that drip bytes.
pub fn receive_until<T: serde::de::DeserializeOwned>(
    stream: &mut UnixStream,
    budget: Duration,
) -> io::Result<T> {
    let deadline = std::time::Instant::now() + budget;
    let mut read_part = |mut output: &mut [u8]| -> io::Result<()> {
        while !output.is_empty() {
            let left = deadline
                .checked_duration_since(std::time::Instant::now())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "request deadline exceeded")
                })?;
            stream.set_read_timeout(Some(left))?;
            match stream.read(output) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "incomplete request",
                    ))
                }
                Ok(n) => {
                    output = &mut output[n..];
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    };
    let mut header = [0; 4];
    read_part(&mut header)?;
    let n = u32::from_be_bytes(header) as usize;
    if n == 0 || n > MAX_FRAME {
        return Err(io::Error::other("invalid frame length"));
    }
    let mut bytes = vec![0; n];
    read_part(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
pub fn call(path: &Path, request: &Request) -> io::Result<Response> {
    let mut socket = UnixStream::connect(path)?;
    let budget = Duration::from_secs(3);
    socket.set_read_timeout(Some(budget))?;
    socket.set_write_timeout(Some(budget))?;
    send(&mut socket, request)?;
    receive_until(&mut socket, budget)
}
pub fn validate(feature: &str, value: &str) -> Result<(), String> {
    let bad = || Err(format!("Invalid {feature} value"));
    if value.len() > 256 || value.contains(['\n', '\r', '\0']) {
        return bad();
    }
    let nums = |n: usize| -> Option<Vec<u32>> {
        let v = value
            .split(',')
            .map(str::parse)
            .collect::<Result<Vec<u32>, _>>()
            .ok()?;
        (v.len() == n).then_some(v)
    };
    let valid = match feature {
        "profile" => {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        }
        "backlight_timeout"
        | "battery_limiter"
        | "battery_calibration"
        | "boot_animation_sound"
        | "lcd_override" => matches!(value, "0" | "1"),
        "usb_charging" => matches!(value, "0" | "10" | "20" | "30"),
        "fan_speed" => nums(2).is_some_and(|n| n.iter().all(|v| *v <= 100)),
        "thermal_fan_mode" => matches!(value, "auto" | "fixed" | "curve"),
        "fan_curve_profile" => matches!(value, "conservative" | "aggressive" | "custom"),
        "four_zone_mode" => nums(7).is_some_and(|n| {
            n[0] <= 7
                && n[1] <= 9
                && n[2] <= 100
                && n[3] <= 2
                && (!(n[0] == 3 || n[0] == 4) || n[3] > 0)
                && n[4..].iter().all(|v| *v <= 255)
        }),
        "per_zone_mode" => {
            let v: Vec<_> = value.split(',').collect();
            v.len() == 5
                && v[..4]
                    .iter()
                    .all(|c| c.len() == 6 && c.bytes().all(|b| b.is_ascii_hexdigit()))
                && v[4].parse::<u32>().is_ok_and(|v| v <= 100)
        }
        "fan_curve_points" => {
            let v: Option<Vec<Vec<u32>>> = value
                .split(',')
                .map(|p| {
                    p.split(':')
                        .map(str::parse)
                        .collect::<Result<Vec<u32>, _>>()
                        .ok()
                })
                .collect();
            v.is_some_and(|v| {
                (2..=8).contains(&v.len())
                    && v.iter().all(|p| {
                        p.len() == 3 && (30..=100).contains(&p[0]) && p[1] <= 100 && p[2] <= 100
                    })
                    && v.windows(2).all(|p| p[0][0] < p[1][0])
            })
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        bad()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_hostile_and_invalid_settings() {
        for (k, v) in [
            ("../evil", "1"),
            ("fan_speed", "-1,20"),
            ("four_zone_mode", "3,2,80,0,1,2,3"),
            ("profile", "performance\n"),
            ("per_zone_mode", "ff0000,ff0000,ff0000,ff0000,101"),
            ("fan_curve_points", "80:50:50,60:30:30"),
        ] {
            assert!(validate(k, v).is_err(), "{k} {v}");
        }
        assert!(validate("four_zone_mode", "3,2,80,1,1,2,3").is_ok());
    }
    #[test]
    fn fragmented_frames_and_limits() {
        let mut bytes = Vec::new();
        send(&mut bytes, &Request::Cycle).unwrap();
        assert!(matches!(
            receive::<Request>(&mut &bytes[..]).unwrap(),
            Request::Cycle
        ));
        assert!(receive::<Request>(&mut &u32::MAX.to_be_bytes()[..]).is_err());
        assert!(receive::<Request>(&mut &bytes[..bytes.len() - 1]).is_err());
    }
}
