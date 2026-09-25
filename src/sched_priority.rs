//! Scheduling opt-outs for the keystroke/echo path.
//!
//! Every keystroke crosses several threads, each waking from a timed wait.
//! Under CPU or memory contention Windows may classify psmux as a background
//! process (EcoQoS) and coalesce its timers to the 15.6ms tick, turning each
//! hop into a visible stall. Everything here is best effort: failures are
//! ignored, and non-Windows builds compile to no-ops.

/// Opt the process out of power throttling. The 1ms timer is requested
/// separately, only while a TimerResolutionGuard is live.
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

static TIMER_HOLDERS: std::sync::Mutex<usize> = std::sync::Mutex::new(0);

/// RAII hold on the 1ms system timer. A raised timer rate costs power
/// machine-wide, so it is held only while someone needs it: the first live
/// guard requests it and dropping the last releases it.
pub struct TimerResolutionGuard(());

impl TimerResolutionGuard {
    pub fn acquire() -> Self {
        // The OS call happens under the lock so a concurrent last-drop cannot
        // interleave its timeEndPeriod between our count and our request.
        let mut n = TIMER_HOLDERS.lock().unwrap_or_else(|e| e.into_inner());
        if *n == 0 {
            set_timer_period(true);
        }
        *n += 1;
        TimerResolutionGuard(())
    }
}

impl Drop for TimerResolutionGuard {
    fn drop(&mut self) {
        let mut n = TIMER_HOLDERS.lock().unwrap_or_else(|e| e.into_inner());
        *n -= 1;
        if *n == 0 {
            set_timer_period(false);
        }
    }
}

/// Number of live TimerResolutionGuards.
pub fn timer_resolution_holders() -> usize {
    *TIMER_HOLDERS.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(windows)]
fn set_timer_period(begin: bool) {
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(uPeriod: u32) -> u32;
        fn timeEndPeriod(uPeriod: u32) -> u32;
    }
    unsafe {
        if begin {
            timeBeginPeriod(1);
        } else {
            timeEndPeriod(1);
        }
    }
}

#[cfg(not(windows))]
fn set_timer_period(_begin: bool) {}
