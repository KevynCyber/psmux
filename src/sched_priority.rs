//! Scheduling opt-outs for the keystroke/echo path.
//!
//! Every keystroke crosses several threads, each waking from a timed wait.
//! Under CPU or memory contention Windows may classify psmux as a background
//! process (EcoQoS) and coalesce its timers to the 15.6ms tick, turning each
//! hop into a visible stall. Everything here is best effort: failures are
//! ignored, and non-Windows builds compile to no-ops.

/// Opt the process out of power throttling and request a 1ms timer.
#[cfg(windows)]
pub fn opt_out_of_power_throttling() {
    const PROCESS_POWER_THROTTLING: i32 = 4; // PROCESS_INFORMATION_CLASS::ProcessPowerThrottling
    const PROCESS_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
    const PROCESS_POWER_THROTTLING_EXECUTION_SPEED: u32 = 0x1;
    const PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION: u32 = 0x4;

    #[repr(C)]
    struct ProcessPowerThrottlingState { version: u32, control_mask: u32, state_mask: u32 }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn SetProcessInformation(hProcess: *mut std::ffi::c_void, class: i32, info: *const std::ffi::c_void, size: u32) -> i32;
    }
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(uPeriod: u32) -> u32;
    }

    // ControlMask names the policies we take control of; StateMask = 0 turns
    // them OFF (always full speed, always honour the requested resolution).
    let state = ProcessPowerThrottlingState {
        version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        control_mask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
        state_mask: 0,
    };
    unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            PROCESS_POWER_THROTTLING,
            &state as *const ProcessPowerThrottlingState as *const std::ffi::c_void,
            std::mem::size_of::<ProcessPowerThrottlingState>() as u32,
        );
        // Never paired with timeEndPeriod: the request is per-process and the
        // OS releases it when the process exits.
        timeBeginPeriod(1);
    }
}

#[cfg(not(windows))]
pub fn opt_out_of_power_throttling() {}

/// Raise the calling thread to THREAD_PRIORITY_ABOVE_NORMAL so the
/// keystroke/echo path is scheduled ahead of competing normal-priority work.
#[cfg(windows)]
pub fn raise_current_thread_priority() {
    const THREAD_PRIORITY_ABOVE_NORMAL: i32 = 1;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> *mut std::ffi::c_void;
        fn SetThreadPriority(hThread: *mut std::ffi::c_void, nPriority: i32) -> i32;
    }

    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    }
}

#[cfg(not(windows))]
pub fn raise_current_thread_priority() {}
