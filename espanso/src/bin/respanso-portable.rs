#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.clamp(0, u8::MAX as i32) as u8),
        Err(error) => {
            report_fatal_error(&error);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<i32, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("unable to resolve portable executable path: {error}"))?;
    let root = executable
        .parent()
        .ok_or_else(|| "portable executable has no parent directory".to_owned())?;

    let core = root.join(core_binary_name());
    if !core.is_file() {
        return Err(format!(
            "rEspanso core executable is missing: {}",
            core.display()
        ));
    }

    let paths = PortablePaths::new(root);
    paths
        .ensure()
        .map_err(|error| format!("unable to prepare portable directories: {error}"))?;

    let user_args = env::args_os().skip(1).collect::<Vec<OsString>>();

    #[cfg(target_os = "windows")]
    if user_args.is_empty() {
        if let Err(error) = cleanup_previous_portable_processes(root) {
            let log_path = paths.runtime.join("rEspanso-bootstrap.log");
            if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(&log_path) {
                let _ = writeln!(log, "[startup-cleanup] {error}");
            }
        }
    }

    let mut command = Command::new(core);
    command
        .current_dir(root)
        .arg("--config_dir")
        .arg(&paths.config_root)
        .arg("--runtime_dir")
        .arg(&paths.runtime)
        .arg("--package_dir")
        .arg(&paths.packages);

    if user_args.is_empty() {
        command.arg("launcher");
    } else {
        command.args(user_args);
    }

    let status = command
        .status()
        .map_err(|error| format!("unable to start rEspanso core: {error}"))?;
    let code = status.code().unwrap_or(1);
    if code != 0 {
        return Err(format!("rEspanso core stopped with exit code {code}"));
    }
    Ok(code)
}

#[cfg(target_os = "windows")]
fn cleanup_previous_portable_processes(root: &Path) -> Result<usize, String> {
    use std::{ffi::c_void, mem::size_of, os::windows::ffi::OsStringExt};

    type Handle = *mut c_void;

    #[repr(C)]
    struct ProcessEntry32W {
        dw_size: u32,
        cnt_usage: u32,
        process_id: u32,
        default_heap_id: usize,
        module_id: u32,
        thread_count: u32,
        parent_process_id: u32,
        priority_class_base: i32,
        flags: u32,
        exe_file: [u16; 260],
    }

    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const WAIT_TIMEOUT_MS: u32 = 2_000;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
        fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            exe_name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetCurrentProcessId() -> u32;
    }

    fn normalized(path: &Path) -> String {
        path.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase()
    }

    fn utf16_z(value: &[u16]) -> OsString {
        let len = value.iter().position(|value| *value == 0).unwrap_or(value.len());
        OsString::from_wide(&value[..len])
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    let invalid_handle = (-1_isize) as Handle;
    if snapshot.is_null() || snapshot == invalid_handle {
        return Err(format!(
            "unable to enumerate stale rEspanso processes: {}",
            std::io::Error::last_os_error()
        ));
    }

    let current_pid = unsafe { GetCurrentProcessId() };
    let root = normalized(root);
    let mut entry: ProcessEntry32W = unsafe { std::mem::zeroed() };
    entry.dw_size = size_of::<ProcessEntry32W>() as u32;
    let mut killed = 0_usize;

    let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while has_entry {
        if entry.process_id != current_pid {
            let name = utf16_z(&entry.exe_file).to_string_lossy().to_ascii_lowercase();
            let is_respanso = matches!(
                name.as_str(),
                "respanso.exe" | "respanso-core.exe" | "espanso.exe"
            );

            if is_respanso {
                let access = PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE;
                let process = unsafe { OpenProcess(access, 0, entry.process_id) };
                if !process.is_null() {
                    let mut image = vec![0_u16; 32_768];
                    let mut image_len = image.len() as u32;
                    let queried = unsafe {
                        QueryFullProcessImageNameW(
                            process,
                            0,
                            image.as_mut_ptr(),
                            &mut image_len,
                        )
                    } != 0;

                    if queried {
                        let process_path = PathBuf::from(OsString::from_wide(
                            &image[..image_len as usize],
                        ));
                        let same_root = process_path
                            .parent()
                            .is_some_and(|parent| normalized(parent) == root);
                        if same_root && unsafe { TerminateProcess(process, 0) } != 0 {
                            let _ = unsafe { WaitForSingleObject(process, WAIT_TIMEOUT_MS) };
                            killed += 1;
                        }
                    }
                    unsafe {
                        CloseHandle(process);
                    }
                }
            }
        }

        entry.dw_size = size_of::<ProcessEntry32W>() as u32;
        has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }

    unsafe {
        CloseHandle(snapshot);
    }
    Ok(killed)
}

fn report_fatal_error(error: &str) {
    let root = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let runtime = root.join("runtime");
    let _ = fs::create_dir_all(&runtime);
    let log_path = runtime.join("rEspanso-bootstrap.log");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(log, "[{timestamp}] {error}");
    }
    show_error_message(&format!(
        "rEspanso не удалось запустить.\n\n{error}\n\nДиагностика: {}",
        log_path.display()
    ));
}

#[cfg(target_os = "windows")]
fn show_error_message(message: &str) {
    use std::ffi::c_void;

    const MB_OK: u32 = 0;
    const MB_ICONERROR: u32 = 0x10;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }

    let title = "rEspanso\0".encode_utf16().collect::<Vec<_>>();
    let message = format!("{message}\0").encode_utf16().collect::<Vec<_>>();
    unsafe {
        let _ = MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_error_message(message: &str) {
    eprintln!("{message}");
}

#[cfg(target_os = "windows")]
fn core_binary_name() -> &'static str {
    "rEspanso-core.exe"
}

#[cfg(not(target_os = "windows"))]
fn core_binary_name() -> &'static str {
    "rEspanso-core"
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortablePaths {
    config_root: PathBuf,
    config: PathBuf,
    matches: PathBuf,
    runtime: PathBuf,
    packages: PathBuf,
}

impl PortablePaths {
    fn new(root: &Path) -> Self {
        Self {
            config_root: root.to_path_buf(),
            config: root.join("config"),
            matches: root.join("match"),
            runtime: root.join("runtime"),
            packages: root.join("packages"),
        }
    }

    fn ensure(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.config)?;
        fs::create_dir_all(&self.matches)?;
        fs::create_dir_all(&self.runtime)?;
        fs::create_dir_all(&self.packages)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_paths_are_relative_to_executable_directory() {
        let paths = PortablePaths::new(Path::new("bundle"));
        assert_eq!(paths.config_root, PathBuf::from("bundle"));
        assert_eq!(paths.config, PathBuf::from("bundle/config"));
        assert_eq!(paths.matches, PathBuf::from("bundle/match"));
        assert_eq!(paths.runtime, PathBuf::from("bundle/runtime"));
        assert_eq!(paths.packages, PathBuf::from("bundle/packages"));
        assert_ne!(paths.config, paths.config_root.join("config/config"));
    }
}
