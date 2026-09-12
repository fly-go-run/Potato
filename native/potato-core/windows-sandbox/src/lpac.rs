//! Verify the actual LPAC security attribute before resuming a child.
//! TokenIsLessPrivilegedAppContainer is not supported by every Windows build.
//! WIN://NOALLAPPPKG is also used by Google Project Zero's NtToken inspector.
use std::{ffi::c_void, io, mem::size_of};
use windows_sys::Win32::Foundation::{HANDLE, NTSTATUS, UNICODE_STRING};

#[repr(C)]
struct Attributes {
    version: u16,
    reserved: u16,
    count: u32,
    attributes: *const Attribute,
}

#[repr(C)]
struct Attribute {
    name: UNICODE_STRING,
    value_type: u16,
    reserved: u16,
    flags: u32,
    count: u32,
    values: *const u64,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySecurityAttributesToken(
        token: HANDLE,
        attributes: *const UNICODE_STRING,
        count: u32,
        buffer: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> NTSTATUS;
}

pub(crate) fn verify(token: HANDLE) -> io::Result<()> {
    let mut name: Vec<u16> = "WIN://NOALLAPPPKG".encode_utf16().collect();
    let attribute = UNICODE_STRING {
        Length: (name.len() * 2) as u16,
        MaximumLength: (name.len() * 2) as u16,
        Buffer: name.as_mut_ptr(),
    };
    // Only one fixed-size integer attribute is requested. Fail closed if the
    // OS returns an unexpected size, shape, name, disabled flag or zero value.
    let mut buffer = [0u64; 512];
    let mut returned = 0;
    let status = unsafe {
        NtQuerySecurityAttributesToken(
            token,
            &attribute,
            1,
            buffer.as_mut_ptr().cast(),
            size_of::<[u64; 512]>() as u32,
            &mut returned,
        )
    };
    if status < 0 {
        return Err(io::Error::other(format!(
            "Query LPAC security attribute failed: NTSTATUS {status:#x}"
        )));
    }
    let invalid = || io::Error::other("Windows did not confirm LPAC restrictions");
    if returned < size_of::<Attributes>() as u32 || returned as usize > size_of_val(&buffer) {
        return Err(invalid());
    }
    let start = buffer.as_ptr() as usize;
    let end = start + returned as usize;
    let contains = |pointer: usize, length: usize| {
        pointer >= start && pointer.checked_add(length).is_some_and(|last| last <= end)
    };
    // The kernel-owned output is bounded before dereferencing its pointers.
    unsafe {
        let info = &*buffer.as_ptr().cast::<Attributes>();
        if info.version != 1
            || info.count != 1
            || !contains(info.attributes as usize, size_of::<Attribute>())
        {
            return Err(invalid());
        }
        let value = info.attributes.read_unaligned();
        if value.value_type != 2
            || value.count != 1
            || value.flags & 0x10 != 0
            || value.name.Length != attribute.Length
            || !contains(value.name.Buffer as usize, name.len() * 2)
            || !contains(value.values as usize, size_of::<u64>())
            || value.values.read_unaligned() != 1
        {
            return Err(invalid());
        }
        for (index, expected) in name.iter().enumerate() {
            if value.name.Buffer.add(index).read_unaligned() != *expected {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn host_token_cannot_pass_lpac_verification() {
        use windows_sys::Win32::{Security::*, System::Threading::*};
        let mut token = std::ptr::null_mut();
        unsafe {
            crate::win::bool_ok(
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token),
                "open host token",
            )
            .unwrap();
        }
        let token = crate::win::Handle(token);
        assert!(super::verify(token.0).is_err());
    }
}
