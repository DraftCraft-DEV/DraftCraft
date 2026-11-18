//! DraftCraft's Windows setup, `draftcraft-windows-<arch>-setup.exe`: the installer (the MSI built
//! from `packaging/windows/draftcraft.wxs`) inside one program.
//!
//! Run by a person, it unpacks the MSI to a temporary folder and opens its wizard (for this account or
//! everyone, the desktop shortcut). Run by DraftCraft's Update Now (`crates/ui-egui/src/self_update.rs`),
//! it updates the installed copy without questions:
//!
//! ```text
//! draftcraft-setup.exe /passive --scope user|machine --after <pid> --relaunch <DraftCraft.exe>
//! ```
//!
//! waits for that DraftCraft (`--after`) to quit, so no file is in use, installs over it in the same
//! context (`ALLUSERS=1` for everyone, as the MSI's install policy asks), then starts DraftCraft again,
//! whether the update went through or was cancelled. The exit code is msiexec's.
//!
//! Options: `/passive` (progress bar only), `/quiet` (nothing shown), `/log <file>` (msiexec's
//! verbose log), `--scope`, `--after`, `--relaunch`.
#![cfg_attr(windows, windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The installer, put here by `build.rs`; empty in a build made without one.
#[cfg_attr(not(windows), allow(dead_code))]
static MSI: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/draftcraft.msi"));

/// How much of the installer is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Ui {
    /// The wizard.
    #[default]
    Full,
    /// A progress bar, no questions.
    Passive,
    /// Nothing.
    Quiet,
}

/// Who the copy being updated is installed for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    User,
    Machine,
}

/// What the command line asked for.
#[derive(Debug, Default, PartialEq)]
struct Options {
    ui: Ui,
    scope: Option<Scope>,
    after: Option<u32>,
    relaunch: Option<PathBuf>,
    log: Option<PathBuf>,
}

const USAGE: &str =
    "DraftCraft Setup\n\nOptions:\n  /passive   show only a progress bar\n  /quiet     show nothing\n  /log FILE  write the installer's log to FILE";

/// Read the command line (without the program's name).
fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut o = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let flag = arg.to_string_lossy().to_ascii_lowercase();
        let mut value = |what: &str| args.next().ok_or_else(|| format!("{what} needs a value"));
        match flag.trim_start_matches(['/', '-']) {
            "passive" | "qb" => o.ui = Ui::Passive,
            "quiet" | "qn" | "q" | "silent" => o.ui = Ui::Quiet,
            "log" | "l*v" => o.log = Some(PathBuf::from(value(&flag)?)),
            "after" => {
                let v = value(&flag)?;
                let pid = v.to_str().and_then(|s| s.trim().parse::<u32>().ok()).ok_or("--after needs a process id")?;
                o.after = Some(pid);
            }
            "relaunch" => o.relaunch = Some(PathBuf::from(value(&flag)?)),
            "scope" => {
                o.scope = match value(&flag)?.to_string_lossy().to_ascii_lowercase().as_str() {
                    "user" => Some(Scope::User),
                    "machine" => Some(Scope::Machine),
                    other => return Err(format!("--scope is user or machine, not {other}")),
                }
            }
            "?" | "h" | "help" => return Err(USAGE.to_string()),
            _ => return Err(format!("unknown option {}\n\n{USAGE}", arg.to_string_lossy())),
        }
    }
    Ok(o)
}

/// msiexec's arguments for installing `msi` as `o` asks.
fn msiexec_args(msi: &Path, o: &Options) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec!["/i".into(), msi.as_os_str().to_owned()];
    match o.ui {
        Ui::Full => {}
        Ui::Passive => a.extend(["/passive".into(), "/norestart".into()]),
        Ui::Quiet => a.extend(["/qn".into(), "/norestart".into()]),
    }
    // For this account the policy leaves ALLUSERS unset; for everyone a silent update passes ALLUSERS=1
    // (packaging/windows/INSTALLATION.md), or it would install a second copy beside the first.
    if o.scope == Some(Scope::Machine) {
        a.push("ALLUSERS=1".into());
    }
    if let Some(log) = &o.log {
        a.push("/l*v".into());
        a.push(log.as_os_str().to_owned());
    }
    a
}

/// Whether `tasklist`'s CSV output lists the process `pid`.
#[cfg_attr(not(windows), allow(dead_code))]
fn lists_process(tasklist_csv: &str, pid: u32) -> bool {
    let quoted = format!("\"{pid}\"");
    tasklist_csv.lines().any(|line| line.split(',').nth(1).is_some_and(|field| field.trim() == quoted))
}

#[cfg(not(windows))]
fn main() {
    let _ = (parse(std::env::args_os().skip(1)), msiexec_args(Path::new(""), &Options::default()), OsStr::new(""));
    eprintln!("DraftCraft Setup installs DraftCraft on Windows only.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    std::process::exit(windows::run());
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::time::{Duration, Instant};

    /// Start a console tool without flashing a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// The longest wait for DraftCraft to quit before installing anyway.
    const MAX_WAIT: Duration = Duration::from_secs(120);
    /// msiexec's exit code for a bad command line.
    const BAD_ARGUMENTS: i32 = 87;

    pub fn run() -> i32 {
        let o = match parse(std::env::args_os().skip(1)) {
            Ok(o) => o,
            Err(e) => {
                tell(Ui::Full, &e);
                return BAD_ARGUMENTS;
            }
        };
        if MSI.is_empty() {
            tell(o.ui, "This setup was built without the DraftCraft installer inside.");
            return 1;
        }
        if let Some(pid) = o.after {
            wait_for_exit(pid);
        }
        let code = install(&o).unwrap_or_else(|e| {
            tell(o.ui, &e);
            1
        });
        if let Some(app) = o.relaunch.as_deref().filter(|p| p.is_file()) {
            let _ = Command::new(app).spawn();
        }
        code
    }

    /// Unpack the installer, run it, clean up; msiexec's exit code.
    fn install(o: &Options) -> Result<i32, String> {
        let dir = std::env::temp_dir().join(format!("DraftCraft-setup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {} ({e}).", dir.display()))?;
        let msi = dir.join("DraftCraft.msi");
        let ran = std::fs::write(&msi, MSI).map_err(|e| format!("Couldn't unpack the installer to {} ({e}).", msi.display())).and_then(|()| {
            Command::new(msiexec()).args(msiexec_args(&msi, o)).status().map_err(|e| format!("Couldn't start Windows Installer ({e})."))
        });
        let _ = std::fs::remove_dir_all(&dir);
        Ok(ran?.code().unwrap_or(1))
    }

    /// Windows Installer, from the system folder rather than whatever `msiexec` the PATH finds first.
    fn msiexec() -> PathBuf {
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from("msiexec.exe"), |root| Path::new(&root).join("System32").join("msiexec.exe"))
    }

    /// Wait until process `pid` has ended, at most [`MAX_WAIT`].
    fn wait_for_exit(pid: u32) {
        let start = Instant::now();
        while start.elapsed() < MAX_WAIT {
            let out = Command::new("tasklist")
                .args([OsStr::new("/FI"), OsStr::new(&format!("PID eq {pid}")), OsStr::new("/FO"), OsStr::new("CSV"), OsStr::new("/NH")])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            match out {
                Ok(out) if out.status.success() => {
                    if !lists_process(&String::from_utf8_lossy(&out.stdout), pid) {
                        return;
                    }
                }
                // No tasklist: give DraftCraft a moment to close, then go ahead.
                _ => {
                    std::thread::sleep(Duration::from_secs(3));
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// Show a message, unless the run is quiet.
    fn tell(ui: Ui, message: &str) {
        if ui == Ui::Quiet {
            return;
        }
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("DraftCraft Setup")
            .set_description(message)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_double_click_opens_the_wizard() {
        let o = parse(args(&[])).unwrap();
        assert_eq!(o, Options::default());
        assert_eq!(msiexec_args(Path::new("C:/t/DraftCraft.msi"), &o), args(&["/i", "C:/t/DraftCraft.msi"]));
    }

    #[test]
    fn an_update_for_everyone_installs_silently_in_the_same_context() {
        let o = parse(args(&[
            "/passive",
            "--scope",
            "machine",
            "--after",
            "4242",
            "--relaunch",
            "C:/Program Files/DraftCraft/DraftCraft/draftcraft.exe",
        ]))
        .unwrap();
        assert_eq!(o.ui, Ui::Passive);
        assert_eq!(o.after, Some(4242));
        assert_eq!(o.relaunch.as_deref(), Some(Path::new("C:/Program Files/DraftCraft/DraftCraft/draftcraft.exe")));
        assert_eq!(msiexec_args(Path::new("x.msi"), &o), args(&["/i", "x.msi", "/passive", "/norestart", "ALLUSERS=1"]));
    }

    #[test]
    fn an_update_for_one_account_leaves_allusers_unset() {
        let o = parse(args(&["-PASSIVE", "--scope", "User", "/log", "C:/t/log.txt"])).unwrap();
        assert_eq!(msiexec_args(Path::new("x.msi"), &o), args(&["/i", "x.msi", "/passive", "/norestart", "/l*v", "C:/t/log.txt"]));
        let quiet = parse(args(&["/qn"])).unwrap();
        assert_eq!(msiexec_args(Path::new("x.msi"), &quiet), args(&["/i", "x.msi", "/qn", "/norestart"]));
    }

    #[test]
    fn a_bad_command_line_is_refused() {
        assert!(parse(args(&["--after"])).is_err());
        assert!(parse(args(&["--after", "-3"])).is_err());
        assert!(parse(args(&["--after", "99999999999"])).is_err());
        assert!(parse(args(&["--scope", "everyone"])).is_err());
        assert!(parse(args(&["/x"])).is_err());
        assert!(parse(args(&["/?"])).unwrap_err().contains("/passive"));
    }

    #[test]
    fn tasklist_output_is_read_by_its_pid_column() {
        let running = "\"draftcraft.exe\",\"4242\",\"Console\",\"1\",\"215,332 K\"\r\n";
        assert!(lists_process(running, 4242));
        assert!(!lists_process(running, 424));
        assert!(!lists_process("INFO: No tasks are running which match the specified criteria.\r\n", 4242));
    }
}
