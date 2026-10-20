// SPDX-License-Identifier: GPL-2.0-or-later
//! PHN16-71 mode-key cycling option.
use crate::ffi;
use core::{
    ffi::{c_char, c_int},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};
#[allow(improper_ctypes)]
unsafe extern "C" {
    static mut __this_module: ffi::module;
}
// These descriptors belong to the kernel module loader's __param section.
// Rust never constructs references to them. The loader initializes the module
// before runtime callbacks can read options; the one writable option is atomic.
static CYCLE: AtomicBool = AtomicBool::new(true);
unsafe extern "C" fn set_cycle(value: *const c_char, _: *const ffi::kernel_param) -> c_int {
    let mut temporary = false;
    let mut parameter: ffi::kernel_param = unsafe { core::mem::zeroed() };
    parameter.__bindgen_anon_1.arg = ptr::addr_of_mut!(temporary).cast();
    let result = unsafe { ffi::param_set_bool(value, &parameter) };
    if result == 0 {
        CYCLE.store(temporary, Ordering::Relaxed);
    }
    result
}
unsafe extern "C" fn get_cycle(buffer: *mut c_char, _: *const ffi::kernel_param) -> c_int {
    unsafe {
        *buffer = if CYCLE.load(Ordering::Relaxed) {
            b'Y'
        } else {
            b'N'
        } as c_char;
        *buffer.add(1) = 0;
    }
    1
}
static CYCLE_OPS: ffi::kernel_param_ops = ffi::kernel_param_ops {
    flags: 1,
    set: Some(set_cycle),
    get: Some(get_cycle),
    free: None,
};
#[used(compiler)]
#[unsafe(link_section = "__param")]
static mut CYCLE_PARAM: ffi::kernel_param = ffi::kernel_param {
    name: c"cycle_gaming_thermal_profile".as_ptr(),
    mod_: ptr::addr_of_mut!(__this_module),
    ops: &CYCLE_OPS,
    perm: 0o644,
    level: -1,
    flags: 0,
    __bindgen_anon_1: ffi::kernel_param__bindgen_ty_1 {
        arg: ptr::null_mut(),
    },
};
#[used(compiler)]
#[unsafe(link_section = ".modinfo")]
static CYCLE_INFO: [u8; b"parmtype=cycle_gaming_thermal_profile:bool\0".len()] =
    *b"parmtype=cycle_gaming_thermal_profile:bool\0";
pub(crate) fn cycle_profiles() -> bool {
    CYCLE.load(Ordering::Relaxed)
}

#[used(compiler)]
#[unsafe(link_section = ".modinfo")]
static DESCRIPTION: [u8;
    b"parm=cycle_gaming_thermal_profile:Cycle profiles with the physical mode key\0".len()] =
    *b"parm=cycle_gaming_thermal_profile:Cycle profiles with the physical mode key\0";
