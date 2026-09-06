//! ZDEP-023: port of portable-pty-psmux 0.9.7 (MIT) `src/win/procthreadattr.rs`.

use super::ffi;
use super::psuedocon::HPCON;
use std::io;

pub struct ProcThreadAttributeList {
    data: Vec<u8>,
}

impl ProcThreadAttributeList {
    pub fn with_capacity(num_attributes: u32) -> io::Result<Self> {
        let mut bytes_required: usize = 0;
        unsafe {
            ffi::InitializeProcThreadAttributeList(
                std::ptr::null_mut(),
                num_attributes,
                0,
                &mut bytes_required,
            )
        };
        // Zero-initialized rather than `Vec::with_capacity` + `set_len`
        // (clippy::uninit_vec is a deny here): the buffer's contents are
        // opaque scratch memory the Win32 APIs below fully own from this
        // point on, so the extra memset is cheap insurance, not waste.
        let mut data = vec![0u8; bytes_required];

        let attr_ptr = data.as_mut_slice().as_mut_ptr() as *mut _;
        let res = unsafe {
            ffi::InitializeProcThreadAttributeList(attr_ptr, num_attributes, 0, &mut bytes_required)
        };
        if res == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { data })
    }

    pub fn as_mut_ptr(&mut self) -> *mut std::ffi::c_void {
        self.data.as_mut_slice().as_mut_ptr() as *mut _
    }

    pub fn set_pty(&mut self, con: HPCON) -> io::Result<()> {
        let res = unsafe {
            ffi::UpdateProcThreadAttribute(
                self.as_mut_ptr(),
                0,
                ffi::PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                con as *const _,
                std::mem::size_of::<HPCON>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if res == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
        unsafe { ffi::DeleteProcThreadAttributeList(self.as_mut_ptr()) };
    }
}
