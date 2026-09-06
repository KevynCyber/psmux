//! ZDEP-023: Win32 `extern "system"` declarations and structs for the
//! ConPTY/process/registry surface this module needs. Ported from what
//! portable-pty-psmux 0.9.7 (MIT) pulled in via winapi; style mirrors
//! src/win32.rs (ZDEP-008: extern decls replacing windows-sys).

use std::ffi::c_void;

pub type HANDLE = *mut c_void;

pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
pub const STILL_ACTIVE: u32 = 259;
pub const INFINITE: u32 = 0xFFFF_FFFF;
pub const S_OK: i32 = 0;

pub const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
pub const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;

pub const STD_INPUT_HANDLE: u32 = (-10i32) as u32;
pub const STD_OUTPUT_HANDLE: u32 = (-11i32) as u32;
pub const STD_ERROR_HANDLE: u32 = (-12i32) as u32;

pub const DUPLICATE_SAME_ACCESS: u32 = 0x0000_0002;
pub const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x0002_0016;

pub const ERROR_BROKEN_PIPE: i32 = 109;
pub const ERROR_HANDLE_EOF: i32 = 38;
pub const ERROR_MORE_DATA: u32 = 234;
pub const ERROR_NO_MORE_ITEMS: u32 = 259;

pub const HKEY_LOCAL_MACHINE: HANDLE = 0x8000_0002u32 as isize as HANDLE;
pub const HKEY_CURRENT_USER: HANDLE = 0x8000_0001u32 as isize as HANDLE;
pub const KEY_READ: u32 = 0x0002_0019;
pub const REG_SZ: u32 = 1;
pub const REG_EXPAND_SZ: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct COORD {
    pub x: i16,
    pub y: i16,
}

#[repr(C)]
pub struct SECURITY_ATTRIBUTES {
    pub n_length: u32,
    pub lp_security_descriptor: *mut c_void,
    pub b_inherit_handle: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct STARTUPINFOW {
    pub cb: u32,
    pub lp_reserved: *mut u16,
    pub lp_desktop: *mut u16,
    pub lp_title: *mut u16,
    pub dw_x: u32,
    pub dw_y: u32,
    pub dw_x_size: u32,
    pub dw_y_size: u32,
    pub dw_x_count_chars: u32,
    pub dw_y_count_chars: u32,
    pub dw_fill_attribute: u32,
    pub dw_flags: u32,
    pub w_show_window: u16,
    pub cb_reserved2: u16,
    pub lp_reserved2: *mut u8,
    pub h_std_input: HANDLE,
    pub h_std_output: HANDLE,
    pub h_std_error: HANDLE,
}

#[repr(C)]
pub struct STARTUPINFOEXW {
    pub startup_info: STARTUPINFOW,
    pub lp_attribute_list: *mut c_void,
}

#[repr(C)]
#[derive(Default)]
pub struct PROCESS_INFORMATION {
    pub h_process: HANDLE,
    pub h_thread: HANDLE,
    pub dw_process_id: u32,
    pub dw_thread_id: u32,
}

impl Default for STARTUPINFOW {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for STARTUPINFOEXW {
    fn default() -> Self {
        Self {
            startup_info: STARTUPINFOW::default(),
            lp_attribute_list: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct OSVERSIONINFOW {
    pub dw_os_version_info_size: u32,
    pub dw_major_version: u32,
    pub dw_minor_version: u32,
    pub dw_build_number: u32,
    pub dw_platform_id: u32,
    pub sz_csd_version: [u16; 128],
}

impl Default for OSVERSIONINFOW {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[link(name = "kernel32")]
extern "system" {
    pub fn CreatePipe(
        h_read_pipe: *mut HANDLE,
        h_write_pipe: *mut HANDLE,
        lp_pipe_attributes: *const SECURITY_ATTRIBUTES,
        n_size: u32,
    ) -> i32;
    // isize, not HANDLE (*mut c_void): matches every other CloseHandle
    // extern decl already in the crate (clashing_extern_declarations is
    // crate-wide per linked symbol, not module-scoped).
    pub fn CloseHandle(handle: isize) -> i32;
    pub fn DuplicateHandle(
        h_source_process_handle: HANDLE,
        h_source_handle: HANDLE,
        h_target_process_handle: HANDLE,
        lp_target_handle: *mut HANDLE,
        dw_desired_access: u32,
        b_inherit_handle: i32,
        dw_options: u32,
    ) -> i32;
    // Buffer as *mut u8 / *const u8: matches every other ReadFile/WriteFile
    // extern decl already in the crate.
    pub fn ReadFile(
        h_file: HANDLE,
        lp_buffer: *mut u8,
        n_number_of_bytes_to_read: u32,
        lp_number_of_bytes_read: *mut u32,
        lp_overlapped: *mut c_void,
    ) -> i32;
    pub fn WriteFile(
        h_file: HANDLE,
        lp_buffer: *const u8,
        n_number_of_bytes_to_write: u32,
        lp_number_of_bytes_written: *mut u32,
        lp_overlapped: *mut c_void,
    ) -> i32;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetExitCodeProcess(h_process: HANDLE, lp_exit_code: *mut u32) -> i32;
    // isize, matching every other TerminateProcess/GetProcessId decl already
    // in the crate.
    pub fn TerminateProcess(h_process: isize, u_exit_code: u32) -> i32;
    pub fn GetProcessId(process: isize) -> u32;
    pub fn WaitForSingleObject(h_handle: HANDLE, dw_milliseconds: u32) -> u32;
    pub fn CreateProcessW(
        lp_application_name: *const u16,
        lp_command_line: *mut u16,
        lp_process_attributes: *const c_void,
        lp_thread_attributes: *const c_void,
        b_inherit_handles: i32,
        dw_creation_flags: u32,
        lp_environment: *const c_void,
        lp_current_directory: *const u16,
        lp_startup_info: *const STARTUPINFOW,
        lp_process_information: *mut PROCESS_INFORMATION,
    ) -> i32;
    pub fn GetStdHandle(n_std_handle: u32) -> HANDLE;
    pub fn SetStdHandle(n_std_handle: u32, h_handle: HANDLE) -> i32;
    pub fn GetModuleHandleW(lp_module_name: *const u16) -> HANDLE;
    pub fn LoadLibraryW(lp_lib_file_name: *const u16) -> HANDLE;
    pub fn GetProcAddress(h_module: HANDLE, lp_proc_name: *const i8) -> *mut c_void;
    pub fn InitializeProcThreadAttributeList(
        lp_attribute_list: *mut c_void,
        dw_attribute_count: u32,
        dw_flags: u32,
        lp_size: *mut usize,
    ) -> i32;
    pub fn UpdateProcThreadAttribute(
        lp_attribute_list: *mut c_void,
        dw_flags: u32,
        attribute: usize,
        lp_value: *const c_void,
        cb_size: usize,
        lp_previous_value: *mut c_void,
        lp_return_size: *mut usize,
    ) -> i32;
    pub fn DeleteProcThreadAttributeList(lp_attribute_list: *mut c_void);
    pub fn ExpandEnvironmentStringsW(lp_src: *const u16, lp_dst: *mut u16, n_size: u32) -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegOpenKeyExW(
        h_key: HANDLE,
        lp_sub_key: *const u16,
        ul_options: u32,
        sam_desired: u32,
        ph_k_result: *mut HANDLE,
    ) -> i32;
    pub fn RegEnumValueW(
        h_key: HANDLE,
        dw_index: u32,
        lp_value_name: *mut u16,
        lpcch_value_name: *mut u32,
        lp_reserved: *mut u32,
        lp_type: *mut u32,
        lp_data: *mut u8,
        lpcb_data: *mut u32,
    ) -> i32;
    pub fn RegQueryInfoKeyW(
        h_key: HANDLE,
        lp_class: *mut u16,
        lpcch_class: *mut u32,
        lp_reserved: *mut u32,
        lpc_sub_keys: *mut u32,
        lpcb_max_sub_key_len: *mut u32,
        lpcb_max_class_len: *mut u32,
        lpc_values: *mut u32,
        lpcb_max_value_name_len: *mut u32,
        lpcb_max_value_len: *mut u32,
        lpcb_security_descriptor: *mut u32,
        lpft_last_write_time: *mut c_void,
    ) -> i32;
    pub fn RegCloseKey(h_key: HANDLE) -> i32;
}
