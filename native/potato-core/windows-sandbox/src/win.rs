use crate::{acl::Lease, quote, Options};
use std::{
    ffi::c_void,
    fs::File,
    io,
    mem::size_of,
    os::windows::io::FromRawHandle,
    path::PathBuf,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Isolation::*, *},
    System::{JobObjects::*, Pipes::*, Threading::*},
};

pub(crate) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn bool_ok(ok: i32, action: &str) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::other(format!(
            "{action}: {}",
            io::Error::last_os_error()
        )))
    } else {
        Ok(())
    }
}
pub(crate) fn code_ok(code: u32, action: &str) -> io::Result<()> {
    if code != 0 {
        Err(io::Error::other(format!(
            "{action}: {}",
            io::Error::from_raw_os_error(code as i32)
        )))
    } else {
        Ok(())
    }
}

pub(crate) struct Handle(pub HANDLE);
unsafe impl Send for Handle {}
impl Handle {
    pub fn new(raw: HANDLE, action: &str) -> io::Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            bool_ok(0, action)?;
        }
        Ok(Self(raw))
    }
    fn into_file(self) -> File {
        let raw = self.0;
        std::mem::forget(self);
        unsafe { File::from_raw_handle(raw) }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub(crate) struct Local(pub *mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

struct Profile {
    name: Vec<u16>,
    sid: PSID,
}
unsafe impl Send for Profile {}
impl Profile {
    fn new() -> io::Result<Self> {
        let name = wide(&format!("Potato.Job.{}", uuid::Uuid::new_v4().simple()));
        let mut sid = null_mut();
        let hr = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if hr < 0 {
            return Err(io::Error::other(format!(
                "CreateAppContainerProfile HRESULT {hr:#x}"
            )));
        }
        Ok(Self { name, sid })
    }
    fn cleanup(&mut self) -> io::Result<()> {
        if self.name.is_empty() {
            return Ok(());
        }
        let hr = unsafe { DeleteAppContainerProfile(self.name.as_ptr()) };
        if hr < 0 {
            return Err(io::Error::other(format!(
                "DeleteAppContainerProfile HRESULT {hr:#x}"
            )));
        }
        self.name.clear();
        Ok(())
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = self.cleanup();
        unsafe {
            FreeSid(self.sid);
        }
    }
}

struct Attributes(Vec<usize>);
impl Attributes {
    fn new(count: u32) -> io::Result<Self> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), count, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(io::Error::other("No process attribute storage size"));
        }
        let mut result = Self(vec![0; bytes.div_ceil(size_of::<usize>())]);
        unsafe {
            bool_ok(
                InitializeProcThreadAttributeList(result.ptr(), count, 0, &mut bytes),
                "initialize process attributes",
            )?;
        }
        Ok(result)
    }
    fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.0.as_mut_ptr().cast()
    }
    // Callers keep every referenced buffer alive until CreateProcessW returns.
    unsafe fn set<T>(&mut self, key: u32, value: &[T]) -> io::Result<()> {
        bool_ok(
            UpdateProcThreadAttribute(
                self.ptr(),
                0,
                key as usize,
                value.as_ptr().cast(),
                std::mem::size_of_val(value),
                null_mut(),
                null_mut(),
            ),
            "set process attribute",
        )
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.ptr());
        }
    }
}

fn capability(name: &str) -> io::Result<Local> {
    unsafe {
        let (mut groups, mut sids) = (null_mut(), null_mut());
        let (mut ngroups, mut nsids) = (0, 0);
        bool_ok(
            DeriveCapabilitySidsFromName(
                wide(name).as_ptr(),
                &mut groups,
                &mut ngroups,
                &mut sids,
                &mut nsids,
            ),
            "derive LPAC capability",
        )?;
        for i in 0..ngroups as usize {
            drop(Local(*groups.add(i)));
        }
        drop(Local(groups.cast()));
        let mut result = None;
        for i in 0..nsids as usize {
            let sid = Local(*sids.add(i));
            if i == 0 {
                result = Some(sid);
            }
        }
        drop(Local(sids.cast()));
        result.ok_or_else(|| io::Error::other("Missing LPAC capability SID"))
    }
}

fn pipe() -> io::Result<(Handle, Handle)> {
    unsafe {
        let (mut read, mut write) = (null_mut(), null_mut());
        bool_ok(
            CreatePipe(&mut read, &mut write, null(), 0),
            "create output pipe",
        )?;
        Ok((Handle(read), Handle(write)))
    }
}

/// Owns the complete process tree, temporary identity and pinned ACL entries.
/// Dropping it kills descendants even after the original shell has exited.
pub struct Process {
    process: Option<Handle>,
    exit_code: Option<i32>,
    job: Handle,
    lease: Option<Lease>,
    profile: Option<Profile>,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
    cleaned: bool,
}

impl Process {
    pub fn spawn(options: &Options) -> io::Result<Self> {
        Self::spawn_cancellable(options, || false)
    }

    pub fn spawn_cancellable(options: &Options, cancelled: impl Fn() -> bool) -> io::Result<Self> {
        if cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Sandbox launch cancelled",
            ));
        }
        for path in [
            &options.program,
            &options.cwd,
            &options.project,
            &options.private,
            &options.scratch,
        ] {
            if !path.is_absolute() || path.to_str().is_none_or(|p| p.contains('\0')) {
                return Err(io::Error::other(
                    "Sandbox paths must be absolute Unicode paths without NUL",
                ));
            }
        }
        if options.args.iter().any(|a| a.contains('\0'))
            || options
                .env
                .iter()
                .any(|(k, v)| k.is_empty() || k.contains(['=', '\0']) || v.contains('\0'))
        {
            return Err(io::Error::other("Invalid process argument/environment"));
        }
        let profile = Profile::new()?;
        let mut lease = Lease::new(profile.sid);
        lease.prepare(options)?;
        let mut caps = vec![capability("registryRead")?];
        if options.network {
            caps.push(capability("internetClient")?);
        }
        let mut cap_entries: Vec<_> = caps
            .iter()
            .map(|sid| SID_AND_ATTRIBUTES {
                Sid: sid.0,
                Attributes: 4, /* SE_GROUP_ENABLED */
            })
            .collect();
        let security = SECURITY_CAPABILITIES {
            AppContainerSid: profile.sid,
            Capabilities: cap_entries.as_mut_ptr(),
            CapabilityCount: cap_entries.len() as u32,
            Reserved: 0,
        };
        let (stdin_read, stdin_write) = pipe()?;
        drop(stdin_write); // EOF, never inherit the app's terminal.
        let (stdout_read, stdout_write) = pipe()?;
        let (stderr_read, stderr_write) = pipe()?;
        unsafe {
            let job = Handle::new(CreateJobObjectW(null(), null()), "create process job")?;
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            limits.BasicLimitInformation.ActiveProcessLimit = 64;
            bool_ok(
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ),
                "configure process job",
            )?;
            let handles = [stdin_read.0, stdout_write.0, stderr_write.0];
            for handle in handles {
                bool_ok(
                    SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT),
                    "prepare stdio handle",
                )?;
            }
            let jobs = [job.0];
            let opt_out = [1u32]; // PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT
            let mut attributes = Attributes::new(4)?;
            attributes.set(
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
                std::slice::from_ref(&security),
            )?;
            attributes.set(
                PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY,
                &opt_out,
            )?;
            attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &handles)?;
            // Atomic membership: no instruction can run outside the job.
            attributes.set(PROC_THREAD_ATTRIBUTE_JOB_LIST, &jobs)?;
            let mut startup: STARTUPINFOEXW = std::mem::zeroed();
            startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = stdin_read.0;
            startup.StartupInfo.hStdOutput = stdout_write.0;
            startup.StartupInfo.hStdError = stderr_write.0;
            startup.lpAttributeList = attributes.ptr();
            let program = options.program.to_str().unwrap();
            let mut argv = vec![quote(program)];
            argv.extend(options.args.iter().map(|arg| quote(arg)));
            let mut command = wide(&argv.join(" "));
            if command.len() > 32767 {
                return Err(io::Error::other(
                    "Windows command line exceeds 32767 UTF-16 units",
                ));
            }
            let mut env: Vec<_> = options.env.iter().collect();
            env.sort_by_key(|(k, _)| k.to_uppercase());
            let mut environment: Vec<u16> = env
                .iter()
                .flat_map(|(k, v)| wide(&format!("{k}={v}")))
                .collect();
            environment.push(0);
            if env.is_empty() {
                environment.push(0);
            }
            let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
            bool_ok(
                CreateProcessW(
                    wide(program).as_ptr(),
                    command.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    CREATE_UNICODE_ENVIRONMENT
                        | EXTENDED_STARTUPINFO_PRESENT
                        | CREATE_NO_WINDOW
                        | CREATE_SUSPENDED,
                    environment.as_ptr().cast(),
                    wide(options.cwd.to_str().unwrap()).as_ptr(),
                    &startup.StartupInfo,
                    &mut pi,
                ),
                "create LPAC process",
            )?;
            let thread = Handle(pi.hThread);
            let process = Self {
                process: Some(Handle(pi.hProcess)),
                exit_code: None,
                job,
                lease: Some(lease),
                profile: Some(profile),
                stdout: Some(stdout_read.into_file()),
                stderr: Some(stderr_read.into_file()),
                cleaned: false,
            };
            // Verify the OS applied AppContainer isolation before any shell code.
            let mut token = null_mut();
            bool_ok(
                OpenProcessToken(process.process.as_ref().unwrap().0, TOKEN_QUERY, &mut token),
                "inspect sandbox token",
            )?;
            let token = Handle(token);
            let (mut is_container, mut returned) = (0u32, 0);
            bool_ok(
                GetTokenInformation(
                    token.0,
                    TokenIsAppContainer,
                    (&mut is_container as *mut u32).cast(),
                    size_of::<u32>() as u32,
                    &mut returned,
                ),
                "verify AppContainer token",
            )?;
            if is_container != 1 {
                return Err(io::Error::other(
                    "Windows did not create an AppContainer token",
                ));
            }
            let mut is_lpac = 0u32;
            bool_ok(
                GetTokenInformation(
                    token.0,
                    TokenIsLessPrivilegedAppContainer,
                    (&mut is_lpac as *mut u32).cast(),
                    size_of::<u32>() as u32,
                    &mut returned,
                ),
                "verify LPAC token",
            )?;
            if is_lpac != 1 {
                return Err(io::Error::other("Windows did not apply LPAC restrictions"));
            }
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Sandbox launch cancelled before resume",
                ));
            }
            if ResumeThread(thread.0) == u32::MAX {
                bool_ok(0, "resume sandbox thread")?;
            }
            Ok(process)
        }
    }

    pub fn try_wait(&self) -> io::Result<Option<i32>> {
        if self.exit_code.is_some() {
            return Ok(self.exit_code);
        }
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| io::Error::other("Sandbox process handle already retired"))?;
        unsafe {
            match WaitForSingleObject(process.0, 0) {
                WAIT_TIMEOUT => Ok(None),
                WAIT_OBJECT_0 => {
                    let mut code = 0;
                    bool_ok(
                        GetExitCodeProcess(process.0, &mut code),
                        "get sandbox exit code",
                    )?;
                    Ok(Some(code as i32))
                }
                _ => Err(io::Error::other("WaitForSingleObject failed")),
            }
        }
    }

    pub fn terminate_tree(&self) -> io::Result<()> {
        unsafe {
            bool_ok(
                TerminateJobObject(self.job.0, 1),
                "terminate sandbox process tree",
            )
        }
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        self.terminate_tree()?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION =
                unsafe { std::mem::zeroed() };
            unsafe {
                bool_ok(
                    QueryInformationJobObject(
                        self.job.0,
                        JobObjectBasicAccountingInformation,
                        (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                        size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                        null_mut(),
                    ),
                    "wait for sandbox descendants",
                )?;
            }
            if accounting.ActiveProcesses == 0 {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other(
                    "Sandbox process tree did not terminate; permissions retained",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.exit_code = self.try_wait()?;
        // Release the terminated process/token before deleting its profile.
        // Keep the cached exit status available to callers after cleanup.
        self.process.take();
        let acl = self.lease.as_mut().map_or(Ok(()), Lease::cleanup);
        let profile = self.profile.as_mut().map_or(Ok(()), Profile::cleanup);
        self.cleaned = true;
        acl.and(profile)
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if self.finish().is_err() && !self.cleaned {
            // Never remove protective ACEs while a process could still be alive.
            // Closing the job is an additional kernel-enforced kill operation.
            if let Some(lease) = self.lease.take() {
                std::mem::forget(lease);
            }
            if let Some(profile) = self.profile.take() {
                std::mem::forget(profile);
            }
        }
    }
}

/// A fixed, bounded launch probe using the same backend as real jobs. It checks
/// project reads, denied project writes and writable scratch, without user code.
pub fn probe() -> io::Result<()> {
    let root = std::env::temp_dir().join(format!("potato-lpac-probe-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root)?;
    let result = (|| {
        let project = root.join("project");
        let scratch = root.join("scratch");
        std::fs::create_dir(&project)?;
        std::fs::create_dir(&scratch)?;
        std::fs::write(project.join("readable.txt"), b"probe")?;
        let system_dir = system_directory()?;
        let system = system_dir
            .parent()
            .ok_or_else(|| io::Error::other("Missing Windows directory"))?
            .display()
            .to_string();
        let options = Options {
            program: system_dir.join("WindowsPowerShell\\v1.0\\powershell.exe"),
            args: crate::powershell_args("$ErrorActionPreference='Stop'; if ([IO.File]::ReadAllText('readable.txt') -ne 'probe') { exit 8 }; try { [IO.File]::WriteAllText('forbidden.txt','bad'); exit 9 } catch [UnauthorizedAccessException] {}; [IO.File]::WriteAllText(($env:TEMP+'\\writable.txt'),'ok'); exit 0"),
            cwd: project.clone(), project: project.clone(), scratch: scratch.clone(), private: root.join("private"), denied: Vec::new(),
            env: [("SystemRoot".into(), system), ("TEMP".into(), scratch.display().to_string())].into(), network: false,
        };
        let mut process = Process::spawn(&options)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let code = loop {
            if let Some(code) = process.try_wait()? {
                break code;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other("Windows sandbox probe timed out"));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        process.finish()?;
        if code != 0
            || project.join("forbidden.txt").exists()
            || !scratch.join("writable.txt").exists()
        {
            return Err(io::Error::other(format!(
                "Windows sandbox enforcement probe failed (exit {code})"
            )));
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(root);
    result
}

pub fn system_directory() -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(io::Error::other("Cannot resolve Windows system directory"));
    }
    Ok(PathBuf::from(
        String::from_utf16(&buffer[..length]).map_err(io::Error::other)?,
    ))
}
