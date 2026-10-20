#![forbid(unsafe_code)]
// SPDX-License-Identifier: GPL-2.0-only
//! Allocation-free firmware protocol validation; testable with host rustc.
pub(crate) fn parse(v: &[u8]) -> Option<u32> {
    if v.is_empty() {
        return None;
    }
    let mut n = 0u32;
    for b in v {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n.checked_mul(10)?.checked_add((b - b'0') as u32)?;
    }
    Some(n)
}
pub(crate) fn numbers<const N: usize>(v: &[u8]) -> Option<[u32; N]> {
    let mut out = [0; N];
    let mut t = v.split(|b| *b == b',');
    for n in &mut out {
        *n = parse(t.next()?)?;
    }
    if t.next().is_some() {
        return None;
    }
    Some(out)
}
pub(crate) fn effect(v: &[u8]) -> Option<[u8; 16]> {
    let n = numbers::<7>(v)?;
    if n[0] > 7
        || n[1] > 9
        || n[2] > 100
        || n[3] > 2
        || n[4..].iter().any(|n| *n > 255)
        || ((n[0] == 3 || n[0] == 4) && n[3] == 0)
    {
        return None;
    }
    Some([
        n[0] as u8, n[1] as u8, n[2] as u8, 0, n[3] as u8, n[4] as u8, n[5] as u8, n[6] as u8, 3,
        1, 0, 0, 0, 0, 0, 0,
    ])
}
pub(crate) fn zones(v: &[u8]) -> Option<([[u8; 3]; 4], u8)> {
    let mut t = v.split(|b| *b == b',');
    let mut colors = [[0; 3]; 4];
    for color in &mut colors {
        let s = t.next()?;
        if s.len() != 6 {
            return None;
        }
        for (i, c) in color.iter_mut().enumerate() {
            let digit = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            };
            *c = digit(s[i * 2])? * 16 + digit(s[i * 2 + 1])?;
        }
    }
    let brightness = parse(t.next()?)?;
    if brightness > 100 || t.next().is_some() {
        return None;
    }
    Some((colors, brightness as u8))
}
#[allow(dead_code)]
pub(crate) fn mode_event(v: &[u8]) -> bool {
    v.len() >= 8 && v[0] == 7 && v[1] == 5
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed() {
        assert!(parse(b"4294967296").is_none());
        assert!(effect(b"3,2,80,0,1,2,3").is_none());
        assert!(effect(b"0,0,80,0,1,2,3,4").is_none());
        assert!(zones(b"ff00zz,000000,000000,000000,100").is_none());
        assert!(!mode_event(&[7, 5]));
    }
    #[test]
    fn wire() {
        assert_eq!(
            effect(b"0,0,80,0,255,128,64").unwrap(),
            [0, 0, 80, 0, 0, 255, 128, 64, 3, 1, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            zones(b"ff0080,00ff00,0000ff,ffffff,70").unwrap().0[0],
            [255, 0, 128]
        );
        for id in [0, 1, 4, 5, 6] {
            assert_eq!(profile_id(profile_name(id).unwrap()), Some(id));
        }
    }
}
