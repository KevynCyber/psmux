//! ZDEP-023: hand-rolled registry read of the HKLM/HKCU `Environment` keys,
//! replacing the `winreg` crate. Ported from portable-pty-psmux 0.9.7 (MIT)
//! `cmdbuilder.rs`'s `get_base_env` Windows half. Malformed registry data
//! (odd byte length, unpaired UTF-16 surrogates) yields fewer entries rather
//! than panicking: this reads externally-controlled machine state.

use super::ffi;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

/// HKLM `System\CurrentControlSet\Control\Session Manager\Environment`, then
/// HKCU `Environment`. HKCU's `Path` is appended to HKLM's with `;`; HKLM's
/// `username` entry is skipped (mirrors the upstream crate's behavior:
/// `username` there is a per-machine artifact, not a real env var).
pub(crate) fn registry_environment() -> Vec<(OsString, OsString)> {
    // Keyed by lowercase name so HKCU can find and extend HKLM's Path; the
    // stored key retains its original (non-lowered) casing for the caller.
    let mut merged: BTreeMap<String, (OsString, OsString)> = BTreeMap::new();

    if let Some(hklm) = open_key(
        ffi::HKEY_LOCAL_MACHINE,
        "System\\CurrentControlSet\\Control\\Session Manager\\Environment",
    ) {
        for (name, value) in enum_values(hklm) {
            let lower = name.to_string_lossy().to_lowercase();
            if lower == "username" {
                continue;
            }
            merged.insert(lower, (name, value));
        }
        unsafe { ffi::RegCloseKey(hklm) };
    }

    if let Some(hkcu) = open_key(ffi::HKEY_CURRENT_USER, "Environment") {
        for (name, value) in enum_values(hkcu) {
            let lower = name.to_string_lossy().to_lowercase();
            let value = if lower == "path" {
                match merged.get(&lower) {
                    Some((_, existing)) => {
                        let mut combined = existing.clone();
                        combined.push(";");
                        combined.push(&value);
                        combined
                    }
                    None => value,
                }
            } else {
                value
            };
            merged.insert(lower, (name, value));
        }
        unsafe { ffi::RegCloseKey(hkcu) };
    }

    merged.into_values().collect()
}

fn to_wide_nul(s: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0);
    wide
}

fn open_key(root: ffi::HANDLE, subkey: &str) -> Option<ffi::HANDLE> {
    let wide = to_wide_nul(subkey);
    let mut handle: ffi::HANDLE = std::ptr::null_mut();
    let res = unsafe { ffi::RegOpenKeyExW(root, wide.as_ptr(), 0, ffi::KEY_READ, &mut handle) };
    if res == 0 {
        Some(handle)
    } else {
        None
    }
}

/// Enumerates every value under `key`, expanding REG_EXPAND_SZ entries.
/// Skips entries that can't be decoded rather than aborting the whole scan.
fn enum_values(key: ffi::HANDLE) -> Vec<(OsString, OsString)> {
    let (max_name_len, max_data_len) = match query_max_lens(key) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    let mut index: u32 = 0;
    loop {
        // +1 for the NUL RegEnumValueW may or may not include, per docs.
        let mut name_buf = vec![0u16; max_name_len as usize + 1];
        let mut name_len = name_buf.len() as u32;
        let mut data_buf = vec![0u8; max_data_len as usize + 2];
        let mut data_len = data_buf.len() as u32;
        let mut value_type: u32 = 0;

        let res = unsafe {
            ffi::RegEnumValueW(
                key,
                index,
                name_buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                &mut value_type,
                data_buf.as_mut_ptr(),
                &mut data_len,
            )
        };

        if res as u32 == ffi::ERROR_NO_MORE_ITEMS {
            break;
        }
        if res != 0 {
            // Any other error on this index: stop rather than loop forever,
            // but keep whatever was already collected.
            break;
        }

        index += 1;

        if value_type != ffi::REG_SZ && value_type != ffi::REG_EXPAND_SZ {
            continue;
        }
        if name_len == 0 {
            continue;
        }

        let name = OsString::from_wide(&name_buf[..name_len as usize]);

        let value = match decode_value(&data_buf[..data_len as usize], value_type) {
            Some(v) => v,
            None => continue,
        };

        out.push((name, value));
    }
    out
}

fn query_max_lens(key: ffi::HANDLE) -> Option<(u32, u32)> {
    let mut num_values: u32 = 0;
    let mut max_name_len: u32 = 0;
    let mut max_value_len: u32 = 0;
    let res = unsafe {
        ffi::RegQueryInfoKeyW(
            key,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut num_values,
            &mut max_name_len,
            &mut max_value_len,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if res != 0 {
        return None;
    }
    Some((max_name_len, max_value_len))
}

/// Decodes a raw registry value's bytes as UTF-16, expanding REG_EXPAND_SZ.
/// Odd byte lengths are malformed data; skip rather than panic.
fn decode_value(bytes: &[u8], value_type: u32) -> Option<OsString> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    // Trim trailing NULs the registry may include.
    while wide.last() == Some(&0) {
        wide.pop();
    }

    if value_type == ffi::REG_EXPAND_SZ {
        Some(expand_environment_strings(&wide))
    } else {
        Some(OsString::from_wide(&wide))
    }
}

fn expand_environment_strings(src: &[u16]) -> OsString {
    let mut src_nul = src.to_vec();
    src_nul.push(0);

    let size = unsafe { ffi::ExpandEnvironmentStringsW(src_nul.as_ptr(), std::ptr::null_mut(), 0) };
    if size == 0 {
        return OsString::from_wide(src);
    }
    let mut buf = vec![0u16; size as usize];
    let written =
        unsafe { ffi::ExpandEnvironmentStringsW(src_nul.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if written == 0 {
        return OsString::from_wide(src);
    }
    let mut buf = &buf[..];
    while buf.last() == Some(&0) {
        buf = &buf[..buf.len() - 1];
    }
    OsString::from_wide(buf)
}
