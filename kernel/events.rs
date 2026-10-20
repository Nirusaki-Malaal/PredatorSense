// SPDX-License-Identifier: GPL-2.0-or-later
//! PHN16-71 events. Parsing and dispatch are safe Rust; input FFI is isolated.
#![forbid(unsafe_code)]
use crate::{gaming, resources, state::State, transport::Result};
pub(crate) fn handle(state: &mut State, bytes: [u8; 8]) -> Result<()> {
    match bytes[0] {
        1 => resources::hotkey(state, bytes[1] as u32),
        7 if matches!(bytes[1], 4 | 5) => gaming::cycle_profile(state),
        8 if bytes[1] <= 1 => {
            let current = bytes[1] as usize;
            gaming::save_power_snapshot(state, 1 - current)?;
            gaming::restore_power_snapshot(state, current)
        }
        11 if bytes[1] <= 1 => gaming::battery_set(state, true, bytes[1] != 0),
        _ => Ok(()),
    }
}
