#![cfg(target_os = "windows")]

// Exercise the production implementation with explicit, isolated registry roots.
// No environment switch or CLI option can redirect production registrations.
#[allow(dead_code)]
#[path = "../src/install.rs"]
mod install;
#[allow(dead_code)]
#[path = "../src/rename.rs"]
mod rename;
#[allow(dead_code)]
#[path = "../src/platform/windows.rs"]
mod windows;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Sandbox {
    directory: PathBuf,
    registry: String,
}

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "malfang-windows-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let directory = std::env::temp_dir().join(&name);
        fs::create_dir(&directory).unwrap();
        Self {
            directory,
            registry: format!(r"HKCU\Software\MalFangTests\{name}"),
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = Command::new("reg")
            .args(["delete", &self.registry, "/f"])
            .output();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn reg(args: &[&str]) -> Output {
    Command::new("reg").args(args).output().unwrap()
}

fn query(key: &str, value: Option<&str>) -> String {
    let args = match value {
        Some(name) => vec!["query", key, "/v", name],
        None => vec!["query", key, "/ve"],
    };
    success(reg(&args))
}

#[test]
fn registers_options_and_commands_and_reinstalls_cleanly() {
    let sandbox = Sandbox::new();
    let file_root = format!(r"{}\files\shell", sandbox.registry);
    let folder_root = format!(r"{}\folders\shell", sandbox.registry);
    let roots = [
        (file_root.as_str(), "file"),
        (folder_root.as_str(), "folder"),
    ];
    // Spaces exercise executable and selected-file quoting without launching it.
    let executable = sandbox
        .directory
        .join("directory with spaces")
        .join("MalFang_tool.exe");
    let executable_text = executable.display().to_string();

    for _ in 0..2 {
        windows::install_context_menu_at_roots(&executable, &roots).unwrap();
        for (root, label) in roots {
            let defang = format!(r"{root}\MalFangToolDefang");
            assert!(query(&defang, None).contains(&format!("Defang {label}")));
            assert!(query(&defang, Some("Icon")).contains(&executable_text));
            assert!(query(&format!(r"{defang}\command"), None)
                .contains(&format!("\"{executable_text}\" --hash sha256 \"%1\"")));

            let refang = format!(r"{root}\MalFangToolRefang");
            assert!(query(&refang, None).contains(&format!("Refang {label}")));
            assert!(query(&format!(r"{refang}\command"), None)
                .contains(&format!("\"{executable_text}\" --refang \"%1\"")));

            let other = format!(r"{root}\MalFangToolDefangOther");
            assert!(query(&other, Some("MUIVerb"))
                .contains(&format!("Defang {label} with other hashes")));
            query(&other, Some("SubCommands"));
            for algorithm in ["md5", "sha1", "sha512", "blake3"] {
                let command = format!(r"{other}\shell\{algorithm}\command");
                assert!(query(&command, None)
                    .contains(&format!("\"{executable_text}\" --hash {algorithm} \"%1\"")));
            }
        }
        // A reinstall must remove stale children of managed keys.
        let stale = format!(r"{file_root}\MalFangToolDefangOther\shell\obsolete");
        if reg(&["query", &stale]).status.success() {
            panic!("reinstall retained a stale submenu");
        }
        success(reg(&["add", &stale, "/f"]));
    }
}

#[test]
fn uninstall_removes_current_and_legacy_entries_but_keeps_unrelated_keys() {
    let sandbox = Sandbox::new();
    let roots: Vec<String> = ["files", "associations", "folders"]
        .iter()
        .map(|target| format!(r"{}\{target}\shell", sandbox.registry))
        .collect();
    let names = [
        "MalFangToolDefang",
        "MalFangToolDefangOther",
        "MalFangToolRefang",
        "HashRenameDefang",
        "HashRenameDefangOther",
        "HashRenameUnfang",
    ];
    for root in &roots {
        for name in names.into_iter().chain(["UnrelatedEntry"]) {
            success(reg(&["add", &format!(r"{root}\{name}\command"), "/f"]));
        }
    }
    let references: Vec<&str> = roots.iter().map(String::as_str).collect();
    // A second uninstall should succeed even though the managed keys are gone.
    for _ in 0..2 {
        windows::uninstall_context_menu_at_roots(&references).unwrap();
        for root in &roots {
            assert!(reg(&["query", root]).status.success());
            assert!(reg(&["query", &format!(r"{root}\UnrelatedEntry\command")])
                .status
                .success());
            for name in names {
                assert!(!reg(&["query", &format!(r"{root}\{name}")]).status.success());
            }
        }
    }
}

#[test]
fn binary_install_child() {
    // Invoked in a subprocess so LOCALAPPDATA never changes in the test runner.
    let Some(expected) = std::env::var_os("MALFANG_TEST_INSTALL_DESTINATION") else {
        return;
    };
    let expected = PathBuf::from(expected);
    assert_eq!(install::context_menu_executable(true).unwrap(), expected);
    assert_eq!(
        fs::read(std::env::current_exe().unwrap()).unwrap(),
        fs::read(expected).unwrap()
    );
}

#[test]
fn optional_binary_install_copies_and_handles_running_from_installed_location() {
    let sandbox = Sandbox::new();
    let destination = sandbox
        .directory
        .join("Programs/MalFang_tool/MalFang_tool.exe");
    // The test executable is a stand-in payload; this doesn't register any menus.
    for program in [std::env::current_exe().unwrap(), destination.clone()] {
        success(
            Command::new(program)
                .args(["--exact", "binary_install_child", "--nocapture"])
                .env("LOCALAPPDATA", &sandbox.directory)
                .env("MALFANG_TEST_INSTALL_DESTINATION", &destination)
                .output()
                .unwrap(),
        );
        assert!(destination.is_file());
    }
}

#[test]
fn default_keeps_current_executable() {
    assert_eq!(
        install::context_menu_executable(false).unwrap(),
        std::env::current_exe().unwrap()
    );
}
