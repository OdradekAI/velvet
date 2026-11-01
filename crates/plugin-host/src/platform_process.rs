#[cfg(any(target_os = "macos", windows))]
use crate::runtime_lease::RuntimeProcessIdentity;

/// Platform-owned lifetime handle for an isolated runtime process tree.
/// Unix uses the existing process-group path; Windows uses a Job Object whose
/// close policy kills descendants when Core disappears.
#[derive(Debug, Default)]
pub struct ProcessSupervisor {
    #[cfg(windows)]
    job: Option<WindowsJob>,
}

impl ProcessSupervisor {
    pub fn new() -> Result<Self, String> {
        #[cfg(windows)]
        {
            WindowsJob::new().map(|job| Self { job: Some(job) })
        }
        #[cfg(not(windows))]
        {
            Ok(Self::default())
        }
    }

    pub fn assign(&self, pid: u32) -> Result<(), String> {
        #[cfg(windows)]
        {
            let Some(job) = self.job.as_ref() else {
                return Err("Windows runtime Job Object was not created".into());
            };
            job.assign(pid)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            Ok(())
        }
    }

    pub fn terminate(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            if let Some(job) = self.job.as_ref() {
                return job.terminate();
            }
        }
        Ok(())
    }
}

/// Returns the strongest process identity available without invoking a shell
/// or a platform command. `None` means the process is absent; an identity with
/// no start token means that the platform denied inspection and must be treated
/// as uncertain by the orphan policy.
#[cfg(any(target_os = "macos", windows))]
pub fn inspect_process(pid: u32) -> Option<RuntimeProcessIdentity> {
    #[cfg(target_os = "macos")]
    {
        inspect_macos_process(pid)
    }
    #[cfg(windows)]
    {
        inspect_windows_process(pid)
    }
}

#[cfg(windows)]
pub fn terminate_process(pid: u32) -> Result<(), String> {
    terminate_windows_process(pid)
}

#[cfg(target_os = "macos")]
fn inspect_macos_process(pid: u32) -> Option<RuntimeProcessIdentity> {
    let info_size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let result = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            info_size,
        )
    };
    if result <= 0 {
        return if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            None
        } else {
            Some(RuntimeProcessIdentity {
                pid,
                process_group_id: None,
                start_token: None,
            })
        };
    }
    if result < info_size {
        return Some(RuntimeProcessIdentity {
            pid,
            process_group_id: None,
            start_token: None,
        });
    }
    let info = unsafe { info.assume_init() };
    Some(RuntimeProcessIdentity {
        pid,
        process_group_id: Some(info.pbi_pgid),
        start_token: Some(format!(
            "{}:{}",
            info.pbi_start_tvsec, info.pbi_start_tvusec
        )),
    })
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsJob {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
// Windows kernel handles are process-wide, reference-counted values that may
// be used by the owner thread and the runtime shutdown thread.
unsafe impl Send for WindowsJob {}

#[cfg(windows)]
impl WindowsJob {
    fn new() -> Result<Self, String> {
        use std::ptr::null;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        let handle = unsafe { CreateJobObjectW(null(), null()) };
        if handle.is_null() {
            return Err(format!("CreateJobObjectW failed: {}", last_error()));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            return Err(format!("SetInformationJobObject failed: {}", last_error()));
        }
        Ok(Self { handle })
    }

    fn assign(&self, pid: u32) -> Result<(), String> {
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA | PROCESS_TERMINATE,
                0,
                pid,
            )
        };
        if process.is_null() {
            return Err(format!(
                "OpenProcess for Job Object failed: {}",
                last_error()
            ));
        }
        let assigned = unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(self.handle, process)
        };
        unsafe { windows_sys::Win32::Foundation::CloseHandle(process) };
        if assigned == 0 {
            return Err(format!("AssignProcessToJobObject failed: {}", last_error()));
        }
        Ok(())
    }

    fn terminate(&self) -> Result<(), String> {
        let terminated =
            unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.handle, 1) };
        if terminated == 0 {
            return Err(format!("TerminateJobObject failed: {}", last_error()));
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle) };
    }
}

#[cfg(windows)]
fn inspect_windows_process(pid: u32) -> Option<RuntimeProcessIdentity> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_INVALID_PARAMETER, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return if unsafe { GetLastError() } == ERROR_INVALID_PARAMETER {
            None
        } else {
            Some(RuntimeProcessIdentity {
                pid,
                process_group_id: None,
                start_token: None,
            })
        };
    }
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let result =
        unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
    unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
    if result == 0 {
        return Some(RuntimeProcessIdentity {
            pid,
            process_group_id: None,
            start_token: None,
        });
    }
    let token = (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    Some(RuntimeProcessIdentity {
        pid,
        process_group_id: None,
        start_token: Some(token.to_string()),
    })
}

#[cfg(windows)]
fn terminate_windows_process(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return Err(format!(
            "OpenProcess for termination failed: {}",
            last_error()
        ));
    }
    let terminated = unsafe { TerminateProcess(handle, 1) };
    unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
    if terminated == 0 {
        return Err(format!("TerminateProcess failed: {}", last_error()));
    }
    Ok(())
}

#[cfg(windows)]
fn last_error() -> u32 {
    unsafe { windows_sys::Win32::Foundation::GetLastError() }
}
