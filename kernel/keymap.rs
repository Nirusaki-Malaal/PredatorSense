// SPDX-License-Identifier: GPL-2.0-or-later
//! Original sparse keymap; ignored duplicate keys remain ignored.
use crate::ffi;
const fn entry(kind: i32, code: u32, key: u32) -> ffi::key_entry {
    ffi::key_entry {
        type_: kind,
        code,
        __bindgen_anon_1: ffi::key_entry__bindgen_ty_1 {
            keycode: key as u16,
        },
    }
}
pub(crate) static KEYMAP: [ffi::key_entry; 36] = [
    entry(1, 0x01, ffi::KEY_WLAN),
    entry(1, 0x03, ffi::KEY_WLAN),
    entry(1, 0x04, ffi::KEY_WLAN),
    entry(1, 0x12, ffi::KEY_BLUETOOTH),
    entry(1, 0x21, ffi::KEY_PROG1),
    entry(1, 0x22, ffi::KEY_PROG2),
    entry(1, 0x23, ffi::KEY_PROG3),
    entry(1, 0x24, ffi::KEY_PROG4),
    entry(1, 0x27, ffi::KEY_HELP),
    entry(1, 0x29, ffi::KEY_PROG3),
    entry(4, 0x41, ffi::KEY_MUTE),
    entry(4, 0x42, ffi::KEY_PREVIOUSSONG),
    entry(4, 0x4d, ffi::KEY_PREVIOUSSONG),
    entry(4, 0x43, ffi::KEY_NEXTSONG),
    entry(4, 0x4e, ffi::KEY_NEXTSONG),
    entry(4, 0x44, ffi::KEY_PLAYPAUSE),
    entry(4, 0x4f, ffi::KEY_PLAYPAUSE),
    entry(4, 0x45, ffi::KEY_STOP),
    entry(4, 0x50, ffi::KEY_STOP),
    entry(4, 0x48, ffi::KEY_VOLUMEUP),
    entry(4, 0x49, ffi::KEY_VOLUMEDOWN),
    entry(4, 0x4a, ffi::KEY_VOLUMEDOWN),
    entry(1, 0x61, ffi::KEY_UNKNOWN),
    entry(4, 0x62, ffi::KEY_BRIGHTNESSUP),
    entry(4, 0x63, ffi::KEY_BRIGHTNESSDOWN),
    entry(1, 0x64, ffi::KEY_SWITCHVIDEOMODE),
    entry(4, 0x81, ffi::KEY_SLEEP),
    entry(1, 0x82, ffi::KEY_TOUCHPAD_TOGGLE),
    entry(4, 0x84, ffi::KEY_KBDILLUMTOGGLE),
    entry(1, ffi::KEY_TOUCHPAD_ON, ffi::KEY_TOUCHPAD_ON),
    entry(1, ffi::KEY_TOUCHPAD_OFF, ffi::KEY_TOUCHPAD_OFF),
    entry(4, 0x83, ffi::KEY_TOUCHPAD_TOGGLE),
    entry(1, 0x85, ffi::KEY_TOUCHPAD_TOGGLE),
    entry(1, 0x86, ffi::KEY_WLAN),
    entry(1, 0x87, ffi::KEY_POWER),
    entry(0, 0, 0),
];
