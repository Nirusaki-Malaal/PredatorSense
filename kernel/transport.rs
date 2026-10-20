// SPDX-License-Identifier: GPL-2.0-or-later
//! Modern WMI transactions. The returned buffer always has exactly one owner.
use crate::ffi;
use core::{ffi::c_void, ptr, slice};
pub(crate) type Result<T> = core::result::Result<T, i32>;
pub(crate) const ACTION: usize = 0;
pub(crate) const GAMING: usize = 1;
pub(crate) const BATTERY: usize = 2;
pub(crate) const EVENTS: usize = 3;
pub(crate) const ENDPOINT_COUNT: usize = 4;
use core::sync::atomic::{AtomicPtr, Ordering};
pub(crate) struct Endpoints {
    devices: [AtomicPtr<ffi::wmi_device>; ENDPOINT_COUNT],
}
impl Endpoints {
    pub(crate) const fn empty() -> Self {
        Self {
            devices: [const { AtomicPtr::new(ptr::null_mut()) }; ENDPOINT_COUNT],
        }
    }
    pub(crate) fn has(&self, kind: usize) -> bool {
        !self.devices[kind].load(Ordering::Relaxed).is_null()
    }
    // Called only by WMI probe/remove while the module STATE guard is held.
    pub(crate) fn attach(&self, kind: usize, device: *mut ffi::wmi_device) -> Result<()> {
        if self.has(kind) {
            return Err(-16);
        }
        // SAFETY: WMI core passes a live device to probe; this reference pins it.
        unsafe {
            ffi::get_device(ptr::addr_of_mut!((*device).dev));
        }
        self.devices[kind].store(device, Ordering::Relaxed);
        Ok(())
    }
    pub(crate) fn detach(&self, device: *mut ffi::wmi_device) {
        for slot in &self.devices {
            if slot.load(Ordering::Relaxed) == device {
                slot.store(ptr::null_mut(), Ordering::Relaxed);
                // SAFETY: exactly balances the reference acquired by attach.
                unsafe {
                    ffi::put_device(ptr::addr_of_mut!((*device).dev));
                }
                break;
            }
        }
    }
    pub(crate) fn invoke(
        &self,
        kind: usize,
        method: u32,
        input: &[u8],
        minimum: usize,
    ) -> Result<Reply> {
        let device = self.devices[kind].load(Ordering::Relaxed);
        if device.is_null() {
            return Err(-19);
        }
        let input = ffi::wmi_buffer {
            length: input.len(),
            data: input.as_ptr().cast_mut().cast(),
        };
        let mut reply = Reply(ffi::wmi_buffer {
            length: 0,
            data: ptr::null_mut(),
        });
        // SAFETY: module registration pins each endpoint until callbacks and the
        // worker are quiescent. The input lives through this synchronous call.
        let status =
            unsafe { ffi::wmidev_invoke_method(device, 0, method, &input, &mut reply.0, minimum) };
        if status < 0 {
            return Err(status);
        }
        if reply.0.data.is_null() || reply.0.length < minimum {
            return Err(-5);
        }
        Ok(reply)
    }
    pub(crate) fn procedure(&self, kind: usize, method: u32, input: &[u8]) -> Result<()> {
        let device = self.devices[kind].load(Ordering::Relaxed);
        if device.is_null() {
            return Err(-19);
        }
        let input = ffi::wmi_buffer {
            length: input.len(),
            data: input.as_ptr().cast_mut().cast(),
        };
        error(unsafe { ffi::wmidev_invoke_procedure(device, 0, method, &input) })
    }
    pub(crate) fn number64(&self, kind: usize, method: u32, input: u64) -> Result<u64> {
        self.invoke(kind, method, &input.to_le_bytes(), 4)?.number()
    }
    pub(crate) fn number32(&self, kind: usize, method: u32, input: u32) -> Result<u64> {
        self.invoke(kind, method, &input.to_le_bytes(), 4)?.number()
    }
    pub(crate) fn misc_get(&self, setting: u8) -> Result<u8> {
        let result = self.number32(GAMING, 23, setting as u32)?;
        if result & 0xff != 0 {
            return Err(-5);
        }
        Ok((result >> 8) as u8)
    }
    pub(crate) fn misc_set(&self, setting: u8, value: u8) -> Result<()> {
        let result = self.number64(GAMING, 22, setting as u64 | ((value as u64) << 8))?;
        if result & 0xff != 0 {
            return Err(-5);
        }
        Ok(())
    }
    pub(crate) fn system_info(&self, command: u32) -> Result<u64> {
        let result = self.number64(GAMING, 5, command as u64)?;
        if result & 0xff != 0 {
            return Err(-5);
        }
        Ok(result)
    }
}
pub(crate) struct Reply(ffi::wmi_buffer);
impl Reply {
    pub(crate) fn bytes(&self) -> &[u8] {
        // SAFETY: invoke/query validated the pointer and minimum size. The WMI
        // API returns a live allocation of length bytes, owned by this guard.
        unsafe { slice::from_raw_parts(self.0.data.cast(), self.0.length) }
    }
    pub(crate) fn number(&self) -> Result<u64> {
        let bytes = self.bytes();
        if bytes.len() < 4 {
            return Err(-5);
        }
        if bytes.len() >= 8 {
            Ok(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
        } else {
            Ok(u32::from_le_bytes(bytes[..4].try_into().unwrap()) as u64)
        }
    }
}
impl Drop for Reply {
    fn drop(&mut self) {
        unsafe {
            ffi::kfree(self.0.data.cast::<c_void>());
        }
    }
}
pub(crate) fn error(status: i32) -> Result<()> {
    if status < 0 {
        Err(status)
    } else {
        Ok(())
    }
}
pub(crate) fn pointer_error<T>(pointer: *mut T) -> Result<*mut T> {
    let value = pointer as isize;
    if (-4095..0).contains(&value) {
        Err(value as i32)
    } else if pointer.is_null() {
        Err(-12)
    } else {
        Ok(pointer)
    }
}

/// Read immutable DMI strings from the kernel; no firmware transaction occurs.
pub(crate) fn identity() -> (&'static str, &'static str) {
    fn read(field: ffi::dmi_field) -> &'static str {
        // SAFETY: DMI storage is immutable and remains valid for the system lifetime.
        let pointer = unsafe { ffi::dmi_get_system_info(field as i32) };
        if pointer.is_null() {
            return "";
        }
        unsafe { core::ffi::CStr::from_ptr(pointer) }
            .to_str()
            .unwrap_or("")
    }
    (read(ffi::DMI_SYS_VENDOR), read(ffi::DMI_PRODUCT_NAME))
}
