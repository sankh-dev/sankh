//! Apps started from a launcher, Finder or the Start menu do not get the
//! user's shell `PATH`, so request files would miss tools like `jq` (and on
//! Windows, Git Bash). This rebuilds `PATH` before anything else runs.

use std::ffi::OsString;
use std::path::PathBuf;

pub fn fix() {
    let current: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let extra = extra_dirs();
    let mut dirs: Vec<PathBuf> = Vec::new();
    for d in extra.into_iter().chain(current.iter().cloned()) {
        if !d.as_os_str().is_empty() && !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    if dirs == current {
        return;
    }
    if let Ok(joined) = std::env::join_paths(dirs) {
        set_path(joined);
    }
}

fn set_path(value: OsString) {
    // SAFETY: called first thing in `main`, before any other thread exists.
    unsafe { std::env::set_var("PATH", value) };
}

#[cfg(unix)]
fn extra_dirs() -> Vec<PathBuf> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const MARK: &str = "__SANKH_PATH__";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let Ok(mut child) = Command::new(&shell)
        .args(["-ilc", &format!("printf '{MARK}%s{MARK}' \"$PATH\"")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
    let Ok(out) = child.wait_with_output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.split(MARK)
        .nth(1)
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default()
}

#[cfg(windows)]
fn extra_dirs() -> Vec<PathBuf> {
    use std::path::Path;

    let has_bash = std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|d| {
            !d.to_string_lossy()
                .to_ascii_lowercase()
                .contains("system32")
                && d.join("bash.exe").is_file()
        })
    });
    if has_bash {
        return Vec::new();
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&p) {
            if d.join("git.exe").is_file()
                && let Some(root) = d.parent()
            {
                roots.push(root.to_path_buf());
            }
        }
    }
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(p) = std::env::var_os(var) {
            roots.push(Path::new(&p).join("Git"));
        }
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        roots.push(Path::new(&p).join("Programs").join("Git"));
    }

    roots
        .into_iter()
        .find(|r| r.join("bin").join("bash.exe").is_file())
        .map(|r| {
            vec![
                r.join("bin"),
                r.join("usr").join("bin"),
                r.join("mingw64").join("bin"),
            ]
        })
        .unwrap_or_default()
}

#[cfg(not(any(unix, windows)))]
fn extra_dirs() -> Vec<PathBuf> {
    Vec::new()
}
