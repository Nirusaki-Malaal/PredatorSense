// SPDX-License-Identifier: GPL-2.0-or-later
//! PHN16-71 driver. Kernel C ownership stays at this boundary.
use core::{
    ffi::{c_int, c_void},
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};
use kernel::prelude::*;
mod event_queue;
mod events;
mod ffi;
mod gaming;
mod keymap;
mod model;
mod parameters;
mod policy;
mod protocol;
mod resources;
mod state;
mod sysfs;
mod thermal_state;
mod transport;
mod wake;
use state::State;
use transport::{
    error, pointer_error, Result as DriverResult, ACTION, BATTERY, ENDPOINT_COUNT, EVENTS, GAMING,
};
module! {
    type: PredatorSense,
    name: "predatrix_sense",
    authors: ["Nirusaki"],
    description: "Predator PHN16-71 Rust driver",
    license: "GPL",
    alias: ["wmi:676AA15E-6A47-4D9F-A2CC-1E6D18D14026"],
}
kernel::sync::global_lock! {
    // SAFETY: initialized exactly once by Module::init before WMI registration.
    unsafe(uninit) static STATE: Mutex<State> = State::new();
}
static READY: AtomicBool = AtomicBool::new(false);
static INITIALIZING: AtomicBool = AtomicBool::new(true);
static WORKER: AtomicPtr<ffi::task_struct> = AtomicPtr::new(ptr::null_mut());
static QUEUE: event_queue::EventQueue<32> = event_queue::EventQueue::new();
fn lock() -> kernel::sync::GlobalGuard<STATE> {
    STATE.lock()
}
pub(crate) fn with_state<T>(
    operation: impl FnOnce(&mut State) -> DriverResult<T>,
) -> DriverResult<T> {
    if !READY.load(Ordering::Acquire) {
        return Err(-19);
    }
    // Subsystem removal can wait for a callback while holding STATE. Callbacks
    // therefore must not block on it: a busy operation is retried by userspace.
    let mut state = STATE.try_lock().ok_or(-16)?;
    if !state.ready {
        return Err(-19);
    }
    operation(&mut state)
}
unsafe extern "C" fn notify(_: *mut ffi::wmi_device, data: *const ffi::wmi_buffer) {
    if data.is_null() {
        return;
    }
    // SAFETY: WMI owns this buffer until the notification callback returns.
    let data = unsafe { &*data };
    if data.data.is_null() || data.length < 8 {
        return;
    }
    let mut bytes = [0u8; 8];
    unsafe {
        ptr::copy_nonoverlapping(data.data.cast::<u8>(), bytes.as_mut_ptr(), 8);
    }
    if QUEUE.push(u64::from_le_bytes(bytes)) {
        wake::signal();
    } else {
        pr_warn!("PHN16-71 event queue full\n");
    }
}
const fn id(guid: &[u8; 36], index: usize) -> ffi::wmi_device_id {
    let mut text = [0; 37];
    let mut offset = 0;
    while offset < 36 {
        text[offset] = guid[offset] as core::ffi::c_char;
        offset += 1;
    }
    ffi::wmi_device_id {
        guid_string: text,
        context: (index + 1) as *const c_void,
    }
}
const END_ID: ffi::wmi_device_id = ffi::wmi_device_id {
    guid_string: [0; 37],
    context: ptr::null(),
};
const METHOD_IDS: &'static [ffi::wmi_device_id; 4] = &[
    id(b"61EF69EA-865C-4BC3-A502-A0DEBA0CB531", ACTION),
    id(b"7A4DDFE7-5B5D-40B4-8595-4408E0CC7F56", GAMING),
    id(b"79772EC5-04B1-4BFD-843C-61E7F77B6CC9", BATTERY),
    END_ID,
];
const EVENT_IDS: &'static [ffi::wmi_device_id; 2] =
    &[id(b"676AA15E-6A47-4D9F-A2CC-1E6D18D14026", EVENTS), END_ID];
unsafe extern "C" fn probe(device: *mut ffi::wmi_device, context: *const c_void) -> c_int {
    let Some(index) = (context as usize).checked_sub(1) else {
        return -22;
    };
    if index >= ENDPOINT_COUNT {
        return -22;
    }
    let mut state = lock();
    if let Err(error) = state.endpoints.attach(index, device) {
        return error;
    }
    if !INITIALIZING.load(Ordering::Acquire)
        && !state.platform().is_null()
        && !state.ready
        && (0..ENDPOINT_COUNT).all(|index| state.endpoints.has(index))
    {
        if let Err(error) = resources::register(&mut state) {
            resources::unregister(&mut state);
            pr_warn!("PHN16-71 registration failed: {}\n", error);
        } else {
            state.ready = true;
            READY.store(true, Ordering::Release);
        }
    }
    0
}
unsafe extern "C" fn remove(device: *mut ffi::wmi_device) {
    READY.store(false, Ordering::Release);
    let mut state = lock();
    if state.ready {
        resources::unregister(&mut state);
    }
    state.endpoints.detach(device);
}
static mut METHOD_DRIVER: ffi::wmi_driver = unsafe { core::mem::zeroed() };
static mut EVENT_DRIVER: ffi::wmi_driver = unsafe { core::mem::zeroed() };
static mut PLATFORM_DRIVER: ffi::platform_driver = unsafe { core::mem::zeroed() };
unsafe extern "C" fn suspend(_: *mut ffi::device) -> c_int {
    resources::suspend(&mut lock()).map_or_else(|error| error, |_| 0)
}
unsafe extern "C" fn resume(_: *mut ffi::device) -> c_int {
    resources::resume(&mut lock()).map_or_else(|error| error, |_| 0)
}
const fn pm_ops() -> ffi::dev_pm_ops {
    let mut ops: ffi::dev_pm_ops = unsafe { core::mem::zeroed() };
    ops.suspend = Some(suspend);
    ops.freeze = Some(suspend);
    ops.poweroff = Some(suspend);
    ops.resume = Some(resume);
    ops.thaw = Some(resume);
    ops.restore = Some(resume);
    ops
}
const PM_OPS: &'static ffi::dev_pm_ops = &pm_ops();
unsafe extern "C" fn worker(_: *mut c_void) -> c_int {
    let mut last_curve = 0;
    while !unsafe { ffi::kthread_should_stop() } {
        let mut delay = 5000;
        if let Some(mut state) = STATE.try_lock() {
            if state.ready && !state.suspended {
                // Bound work under STATE even if firmware keeps producing events.
                for _ in 0..32 {
                    let Some(event) = QUEUE.pop() else { break };
                    if let Err(error) = events::handle(&mut state, event.to_le_bytes()) {
                        pr_warn!("PHN16-71 event failed: {}\n", error);
                    }
                }
                let now = unsafe { ffi::ktime_get_mono_fast_ns() };
                if now.saturating_sub(last_curve) >= policy::CURVE_INTERVAL_SECONDS * 1_000_000_000
                {
                    last_curve = now;
                    if let Err(error) = gaming::curve_tick(&mut state) {
                        pr_warn!("Fan curve failed: {}\n", error);
                    }
                }
            }
        } else {
            delay = 1;
        }
        wake::wait(delay);
    }
    0
}
struct PredatorSense {
    methods: bool,
    events: bool,
    platform: bool,
    wake: bool,
}
impl PredatorSense {
    fn initialize(&mut self, module: &'static kernel::ThisModule) -> DriverResult<()> {
        let (vendor, product) = transport::identity();
        if !model::supported(vendor, product) {
            return Err(-19);
        }
        let existing = unsafe {
            ffi::bus_find_device(
                ptr::addr_of!(ffi::platform_bus_type),
                ptr::null_mut(),
                c"acer-wmi".as_ptr().cast(),
                Some(ffi::device_match_name),
            )
        };
        if !existing.is_null() {
            unsafe {
                ffi::put_device(existing);
            }
            return Err(-16);
        }
        wake::initialize().map_err(|error| error.to_errno())?;
        self.wake = true;
        let driver = ptr::addr_of_mut!(METHOD_DRIVER);
        unsafe {
            (*driver).driver.name = c"predator-phn16-methods".as_ptr();
            (*driver).driver.probe_type = ffi::PROBE_FORCE_SYNCHRONOUS;
            (*driver).id_table = METHOD_IDS.as_ptr();
            (*driver).no_singleton = true;
            (*driver).probe = Some(probe);
            (*driver).remove = Some(remove);
        }
        error(unsafe { ffi::__wmi_driver_register(driver, module.as_ptr().cast()) })?;
        self.methods = true;
        {
            let state = lock();
            if !(state.endpoints.has(ACTION)
                && state.endpoints.has(GAMING)
                && state.endpoints.has(BATTERY))
            {
                return Err(-19);
            }
            // Preserve Acer's setup order: application mode before events.
            for (status, mask) in [(0x10, 0x10), (1, 1)] {
                let reply = state.endpoints.invoke(
                    ACTION,
                    1,
                    &[1, 255, 255, 255, 255, status, mask, 0],
                    4,
                )?;
                if reply.bytes()[0] != 0 || reply.bytes()[1] != 0 {
                    return Err(-5);
                }
            }
        }
        let driver = ptr::addr_of_mut!(EVENT_DRIVER);
        unsafe {
            (*driver).driver.name = c"predator-phn16-events".as_ptr();
            (*driver).driver.probe_type = ffi::PROBE_FORCE_SYNCHRONOUS;
            (*driver).id_table = EVENT_IDS.as_ptr();
            (*driver).no_singleton = true;
            (*driver).probe = Some(probe);
            (*driver).remove = Some(remove);
            (*driver).notify_new = Some(notify);
            (*driver).min_event_size = 8;
        }
        error(unsafe { ffi::__wmi_driver_register(driver, module.as_ptr().cast()) })?;
        self.events = true;
        if !lock().endpoints.has(EVENTS) {
            return Err(-19);
        }
        let driver = ptr::addr_of_mut!(PLATFORM_DRIVER);
        unsafe {
            (*driver).driver.name = c"acer-wmi".as_ptr();
            (*driver).driver.pm = PM_OPS;
        }
        error(unsafe {
            ffi::__platform_driver_register(
                driver,
                module.as_ptr().cast(),
                c"predatrix_sense".as_ptr(),
            )
        })?;
        self.platform = true;
        let device =
            pointer_error(unsafe { ffi::platform_device_alloc(c"acer-wmi".as_ptr(), -1) })?;
        if let Err(error) = error(unsafe { ffi::platform_device_add(device) }) {
            unsafe {
                ffi::platform_device_put(device);
            }
            return Err(error);
        }
        {
            let mut state = lock();
            state.platform.store(device, Ordering::Relaxed);
            resources::register(&mut state)?;
            state.ready = true;
            READY.store(true, Ordering::Release);
        }
        INITIALIZING.store(false, Ordering::Release);
        let task = pointer_error(unsafe {
            ffi::kthread_create_on_node(
                Some(worker),
                ptr::null_mut(),
                -1,
                c"predator-events".as_ptr(),
            )
        })?;
        WORKER.store(task, Ordering::Release);
        unsafe {
            ffi::wake_up_process(task);
        }
        pr_info!("PHN16-71 driver registered\n");
        Ok(())
    }
}
impl kernel::Module for PredatorSense {
    fn init(module: &'static kernel::ThisModule) -> kernel::error::Result<Self> {
        // SAFETY: exactly once, before any callback can acquire STATE.
        unsafe {
            STATE.init();
        }
        let mut driver = Self {
            methods: false,
            events: false,
            platform: false,
            wake: false,
        };
        driver
            .initialize(module)
            .map_err(kernel::error::Error::from_errno)?;
        Ok(driver)
    }
}
impl Drop for PredatorSense {
    fn drop(&mut self) {
        READY.store(false, Ordering::Release);
        INITIALIZING.store(true, Ordering::Release);
        // Drain notifications before freeing their completion or endpoint refs.
        if self.events {
            unsafe {
                ffi::wmi_driver_unregister(ptr::addr_of_mut!(EVENT_DRIVER));
            }
        }
        wake::stop_waiting();
        let task = WORKER.swap(ptr::null_mut(), Ordering::AcqRel);
        if !task.is_null() {
            unsafe {
                ffi::kthread_stop(task);
            }
        }
        let device = {
            let mut state = lock();
            if !state.platform().is_null() {
                resources::unregister(&mut state);
            }
            state.platform.swap(ptr::null_mut(), Ordering::Relaxed)
        };
        if !device.is_null() {
            unsafe {
                ffi::platform_device_unregister(device);
            }
        }
        if self.platform {
            unsafe {
                ffi::platform_driver_unregister(ptr::addr_of_mut!(PLATFORM_DRIVER));
            }
        }
        if self.methods {
            unsafe {
                ffi::wmi_driver_unregister(ptr::addr_of_mut!(METHOD_DRIVER));
            }
        }
        if self.wake {
            wake::release();
        }
    }
}
#[used(compiler)]
#[unsafe(link_section = ".modinfo")]
static VERSION: [u8; 14] = *b"version=0.6.1\0";
