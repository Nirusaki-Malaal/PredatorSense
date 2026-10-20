// SPDX-License-Identifier: GPL-2.0-or-later
//! Durable worker wakeups: completion credits cannot be lost before sleeping.
//!
//! The module owns one kernel allocation. Notifications are drained and the
//! worker is joined before that allocation is released. No references escape.
use core::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};
use kernel::{alloc::KBox, bindings, prelude::*};
static COMPLETION: AtomicPtr<bindings::completion> = AtomicPtr::new(ptr::null_mut());
pub(crate) fn initialize() -> Result<()> {
    // SAFETY: all-zero bit patterns are valid for the C storage; the kernel
    // initializes the completion before either thread can see its pointer.
    let mut value = KBox::new(
        unsafe { core::mem::zeroed::<bindings::completion>() },
        GFP_KERNEL,
    )?;
    unsafe {
        bindings::init_completion(&mut *value);
    }
    COMPLETION.store(KBox::into_raw(value), Ordering::Release);
    Ok(())
}
pub(crate) fn signal() {
    let pointer = COMPLETION.load(Ordering::Acquire);
    if !pointer.is_null() {
        // SAFETY: initialization precedes WMI registration; removal drains all
        // callbacks before release. complete() is valid in notification context.
        unsafe {
            bindings::complete(pointer);
        }
    }
}
pub(crate) fn wait(milliseconds: u32) {
    let pointer = COMPLETION.load(Ordering::Acquire);
    if !pointer.is_null() {
        let ticks = (milliseconds as u64 * crate::ffi::CONFIG_HZ as u64)
            .div_ceil(1000)
            .max(1);
        // SAFETY: only the module worker waits; it is joined before release.
        unsafe {
            bindings::wait_for_completion_timeout(pointer, ticks as _);
        }
    }
}
pub(crate) fn stop_waiting() {
    let pointer = COMPLETION.load(Ordering::Acquire);
    if !pointer.is_null() {
        // Permanent completion prevents re-sleeping while kthread_stop joins.
        unsafe {
            bindings::complete_all(pointer);
        }
    }
}
pub(crate) fn release() {
    let pointer = COMPLETION.swap(ptr::null_mut(), Ordering::AcqRel);
    if !pointer.is_null() {
        // SAFETY: exactly the allocation returned by into_raw, after all users
        // are quiescent. Reconstruct its sole owner and run normal destruction.
        unsafe {
            drop(KBox::from_raw(pointer));
        }
    }
}
