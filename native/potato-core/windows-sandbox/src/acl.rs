//! Per-job SID entries only. Never grant Everyone/ALL APPLICATION PACKAGES and
//! never restore a stale whole DACL over changes made by another job or user.
use super::win::{bool_ok, code_ok, wide, Handle, Local};
use crate::{secret_name, Options};
use std::{
    io,
    path::Path,
    ptr::{null, null_mut},
    sync::Mutex,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::Threading::*,
};

static ACL_EDIT: Mutex<()> = Mutex::new(());

struct NamedLock(Handle);
impl NamedLock {
    fn acquire() -> io::Result<Self> {
        unsafe {
            let handle = Handle::new(
                CreateMutexW(null(), 0, wide("Local\\Potato.Sandbox.ACL.v1").as_ptr()),
                "open sandbox ACL mutex",
            )?;
            match WaitForSingleObject(handle.0, 5000) {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(handle)),
                _ => Err(io::Error::other("Could not lock sandbox ACL edits")),
            }
        }
    }
}
impl Drop for NamedLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0 .0);
        }
    }
}
const WRITE_RIGHTS: u32 = FILE_WRITE_DATA
    | FILE_APPEND_DATA
    | FILE_WRITE_EA
    | FILE_WRITE_ATTRIBUTES
    | FILE_DELETE_CHILD
    | DELETE
    | WRITE_DAC
    | WRITE_OWNER;

pub(crate) struct Lease {
    handles: Vec<Handle>,
    sid: PSID,
}
// The SID is owned by the enclosing Process's profile. All operations on these
// handles are exclusive; the lease is moved between threads, never shared.
unsafe impl Send for Lease {}

impl Lease {
    pub fn new(sid: PSID) -> Self {
        Self {
            handles: Vec::new(),
            sid,
        }
    }

    pub fn prepare(&mut self, options: &Options) -> io::Result<()> {
        if options.project.starts_with(&options.scratch)
            || options.scratch.starts_with(&options.project)
            || !options.cwd.starts_with(&options.project)
        {
            return Err(io::Error::other(
                "Sandbox project, scratch and cwd scopes overlap or escape",
            ));
        }
        // LPAC still has an OS-defined baseline. A path deny outside the project
        // cannot be represented by withholding a project grant. Fail closed;
        // Potato's orchestrator also forbids host fallback with deny rules.
        if options
            .denied
            .iter()
            .any(|p| !p.starts_with(&options.project))
        {
            return Err(io::Error::other(
                "Windows sandbox cannot enforce external directory deny rules",
            ));
        }
        let mut pending = vec![(options.project.clone(), false)];
        while let Some((path, parent_denied)) = pending.pop() {
            if self.handles.len() >= 20_000 {
                return Err(io::Error::other(
                    "Sandbox project exceeds 20000 pinned entries",
                ));
            }
            let denied = parent_denied
                || path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(secret_name)
                || options.denied.iter().any(|p| path.starts_with(p))
                || (path.starts_with(&options.private)
                    && !options.project.starts_with(&options.private));
            let (handle, info) = open_pinned(&path, false)?;
            // An alias may refer to data outside the project. No access grant is
            // installed on its target, and no links/junctions are traversed.
            let alias = info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
                    && info.nNumberOfLinks > 1);
            if alias {
                return Err(io::Error::other("Sandbox project contains a reparse point or hardlink; use a link-free project or a reviewed alternative"));
            }
            self.change(
                handle,
                if denied {
                    0
                } else {
                    FILE_GENERIC_READ | FILE_GENERIC_EXECUTE
                },
                if denied {
                    FILE_ALL_ACCESS
                } else {
                    WRITE_RIGHTS
                },
                0,
            )?;
            if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                for entry in std::fs::read_dir(&path)? {
                    pending.push((entry?.path(), denied));
                }
            }
        }
        let mut pending = vec![options.scratch.clone()];
        while let Some(path) = pending.pop() {
            if self.handles.len() >= 20_000 {
                return Err(io::Error::other("Sandbox exceeds 20000 pinned entries"));
            }
            let (scratch, info) = open_pinned(&path, true)?;
            if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
                    && info.nNumberOfLinks > 1)
            {
                return Err(io::Error::other(
                    "Sandbox scratch contains a reparse point or hardlink",
                ));
            }
            if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                for entry in std::fs::read_dir(&path)? {
                    pending.push(entry?.path());
                }
            }
            // Low-integrity descendants can write only this disposable directory.
            // LABEL_SECURITY_INFORMATION touches no owner or discretionary ACEs.
            unsafe {
                let mut descriptor = null_mut();
                bool_ok(
                    ConvertStringSecurityDescriptorToSecurityDescriptorW(
                        wide("S:(ML;OICI;NW;;;LW)").as_ptr(),
                        SDDL_REVISION_1,
                        &mut descriptor,
                        null_mut(),
                    ),
                    "build scratch integrity label",
                )?;
                let descriptor = Local(descriptor);
                bool_ok(
                    SetKernelObjectSecurity(scratch.0, LABEL_SECURITY_INFORMATION, descriptor.0),
                    "set scratch integrity label",
                )?;
            }
            self.change(
                scratch,
                FILE_GENERIC_READ | FILE_GENERIC_EXECUTE | FILE_GENERIC_WRITE | DELETE,
                0,
                OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
            )?;
        }
        Ok(())
    }

    fn change(&mut self, handle: Handle, allow: u32, deny: u32, inherit: u32) -> io::Result<()> {
        // Register before the mutation so rollback covers every successful edit.
        self.handles.push(handle);
        let handle = self.handles.last().unwrap().0;
        edit(handle, self.sid, allow, deny, inherit, false)
    }

    pub fn cleanup(&mut self) -> io::Result<()> {
        let mut first = None;
        for handle in self.handles.drain(..).rev() {
            if let Err(error) = edit(handle.0, self.sid, 0, 0, 0, true) {
                first.get_or_insert(error);
            }
        }
        first.map_or(Ok(()), Err)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn open_pinned(path: &Path, label: bool) -> io::Result<(Handle, BY_HANDLE_FILE_INFORMATION)> {
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::other("Non-Unicode sandbox path"))?;
    if path.contains('\0') {
        return Err(io::Error::other("NUL in sandbox path"));
    }
    unsafe {
        // No FILE_SHARE_DELETE: identity cannot be swapped while grants are live.
        let handle = Handle::new(
            CreateFileW(
                wide(path).as_ptr(),
                READ_CONTROL
                    | WRITE_DAC
                    | FILE_READ_ATTRIBUTES
                    | if label { WRITE_OWNER } else { 0 },
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            ),
            "pin sandbox path",
        )?;
        let mut info = std::mem::zeroed();
        bool_ok(
            GetFileInformationByHandle(handle.0, &mut info),
            "inspect sandbox path",
        )?;
        Ok((handle, info))
    }
}

fn edit(
    handle: HANDLE,
    sid: PSID,
    allow: u32,
    deny: u32,
    inherit: u32,
    revoke: bool,
) -> io::Result<()> {
    let _lock = ACL_EDIT
        .lock()
        .map_err(|_| io::Error::other("Sandbox ACL lock poisoned"))?;
    let _cross_process_lock = NamedLock::acquire()?;
    unsafe {
        let mut descriptor = null_mut();
        let mut old_acl = null_mut();
        code_ok(
            GetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old_acl,
                null_mut(),
                &mut descriptor,
            ),
            "read sandbox DACL",
        )?;
        let _descriptor = Local(descriptor);
        if old_acl.is_null() {
            return Err(io::Error::other("Sandbox refuses a null/unrestricted DACL"));
        }
        if revoke {
            // REVOKE_ACCESS does not promise to remove deny ACEs. Remove only
            // our unique SID's allow/deny entries (including inherited scratch
            // entries) from the current DACL; leave every other ACE untouched.
            for index in (0..(*old_acl).AceCount as u32).rev() {
                let mut ace = null_mut();
                bool_ok(GetAce(old_acl, index, &mut ace), "read sandbox ACE")?;
                let header = &*ace.cast::<ACE_HEADER>();
                if matches!(header.AceType, 0 | 1) {
                    // ACCESS_ALLOWED/DENIED_ACE_TYPE
                    let ace_sid = (&(*ace.cast::<ACCESS_ALLOWED_ACE>()).SidStart as *const u32)
                        .cast_mut()
                        .cast();
                    if EqualSid(ace_sid, sid) != 0 {
                        bool_ok(DeleteAce(old_acl, index), "remove sandbox SID entry")?;
                    }
                }
            }
            let mut info: ACL_SIZE_INFORMATION = std::mem::zeroed();
            bool_ok(
                GetAclInformation(
                    old_acl,
                    (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                    std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                    AclSizeInformation,
                ),
                "size cleaned sandbox ACL",
            )?;
            (*old_acl).AclSize = info.AclBytesInUse as u16;
            return apply_dacl(handle, old_acl);
        }
        let entry = |mask, mode| EXPLICIT_ACCESS_W {
            grfAccessPermissions: mask,
            grfAccessMode: mode,
            grfInheritance: inherit,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: sid.cast(),
            },
        };
        let mut entries = Vec::new();
        if deny != 0 {
            entries.push(entry(deny, DENY_ACCESS));
        }
        if allow != 0 {
            entries.push(entry(allow, GRANT_ACCESS));
        }
        let mut acl = null_mut();
        code_ok(
            SetEntriesInAclW(entries.len() as u32, entries.as_ptr(), old_acl, &mut acl),
            "merge sandbox DACL",
        )?;
        let acl = Local(acl.cast());
        apply_dacl(handle, acl.0.cast())
    }
}

unsafe fn apply_dacl(handle: HANDLE, acl: *mut ACL) -> io::Result<()> {
    let mut sd: SECURITY_DESCRIPTOR = std::mem::zeroed();
    bool_ok(
        InitializeSecurityDescriptor((&mut sd as *mut SECURITY_DESCRIPTOR).cast(), 1),
        "initialize DACL",
    )?;
    bool_ok(
        SetSecurityDescriptorDacl((&mut sd as *mut SECURITY_DESCRIPTOR).cast(), 1, acl, 0),
        "build DACL",
    )?;
    // Unlike SetSecurityInfo, do not recursively rewrite child DACLs. Every
    // existing entry is pinned and edited explicitly; scratch inherits only
    // into new files created during the job.
    bool_ok(
        SetKernelObjectSecurity(
            handle,
            DACL_SECURITY_INFORMATION,
            (&mut sd as *mut SECURITY_DESCRIPTOR).cast(),
        ),
        "apply sandbox DACL",
    )
}

#[cfg(test)]
pub(crate) fn dacl_for_test(path: &Path) -> Vec<u8> {
    let (handle, _) = open_pinned(path, false).unwrap();
    unsafe {
        let (mut descriptor, mut acl) = (null_mut(), null_mut());
        code_ok(
            GetSecurityInfo(
                handle.0,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut acl,
                null_mut(),
                &mut descriptor,
            ),
            "read test DACL",
        )
        .unwrap();
        let _descriptor = Local(descriptor);
        assert!(!acl.is_null());
        std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize).to_vec()
    }
}
