use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::rename::{self, HashAlgorithm, RenameOperation};

mod actions;
mod refresh;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub(crate) enum FileManager {
    Nautilus,
    #[value(alias = "nemo-desktop")]
    Nemo,
    Caja,
    #[value(alias = "plasma")]
    Dolphin,
    #[value(alias = "xfdesktop")]
    Thunar,
    Xfe,
    Pcmanfm,
    Ding,
}

impl FileManager {
    const ALL: [Self; 8] = [
        Self::Nautilus,
        Self::Nemo,
        Self::Caja,
        Self::Dolphin,
        Self::Thunar,
        Self::Xfe,
        Self::Pcmanfm,
        Self::Ding,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Nautilus => "nautilus",
            Self::Nemo => "nemo",
            Self::Caja => "caja",
            Self::Dolphin => "dolphin",
            Self::Thunar => "thunar",
            Self::Xfe => "xfe",
            Self::Pcmanfm => "pcmanfm",
            Self::Ding => "ding",
        }
    }

    fn selection_var(self) -> Option<&'static str> {
        match self {
            Self::Nautilus | Self::Ding => Some("NAUTILUS_SCRIPT_SELECTED_FILE_PATHS"),
            Self::Nemo => Some("NEMO_SCRIPT_SELECTED_FILE_PATHS"),
            Self::Caja => Some("CAJA_SCRIPT_SELECTED_FILE_PATHS"),
            _ => None,
        }
    }

    fn uses_scripts_menu(self) -> bool {
        matches!(self, Self::Nautilus | Self::Nemo | Self::Caja | Self::Xfe | Self::Ding)
    }

    fn registration_path(self) -> io::Result<PathBuf> {
        Ok(match self {
            Self::Nautilus | Self::Nemo => data_home()?.join(self.name()).join("scripts"),
            // Desktop Icons NG uses the default Nautilus path even with a custom XDG_DATA_HOME.
            Self::Ding => home_dir()?.join(".local/share/nautilus/scripts"),
            Self::Caja => config_home()?.join("caja/scripts"),
            // Xfe uses this path even when XDG_CONFIG_HOME is set.
            Self::Xfe => home_dir()?.join(".config/xfe/scripts"),
            Self::Dolphin => data_home()?.join("kio/servicemenus/malfang-tool.desktop"),
            Self::Thunar => config_home()?.join("Thunar/uca.xml"),
            Self::Pcmanfm => data_home()?.join("file-manager/actions"),
        })
    }

    fn wrappers_dir(self, registration: &Path) -> io::Result<PathBuf> {
        if self.uses_scripts_menu() {
            Ok(registration.to_path_buf())
        } else {
            Ok(data_home()?.join("malfang-tool").join(self.name()))
        }
    }

    fn wrapper_name(self, entry: ScriptEntry) -> &'static str {
        if self.uses_scripts_menu() {
            entry.file_name
        } else {
            entry.id
        }
    }

    fn detected(self, registration: &Path, search_path: Option<&OsStr>) -> bool {
        let has_registration = match self {
            Self::Dolphin | Self::Thunar => registration.is_file(),
            Self::Pcmanfm => registration.join("malfang-tool-defang.desktop").is_file(),
            Self::Ding => return ding_installed(),
            _ => registration.is_dir(),
        };
        let desktop = match self {
            Self::Thunar => Some("xfdesktop"),
            Self::Dolphin => Some("plasmashell"),
            Self::Nemo => Some("nemo-desktop"),
            _ => None,
        };
        has_registration
            || on_path(search_path, self.name())
            || desktop.is_some_and(|name| on_path(search_path, name))
    }

    fn refresh_hint(self) -> Option<&'static str> {
        match self {
            Self::Thunar => Some("Thunar: if needed, run `thunar --quit` and reopen it. Xfce desktop icons: restart xfdesktop or sign out and back in."),
            Self::Dolphin => Some("KDE Plasma: right-click a file icon in Folder View."),
            Self::Ding => Some("GNOME: with Desktop Icons NG enabled, right-click a file icon and open Scripts."),
            _ => None,
        }
    }
}

fn on_path(search_path: Option<&OsStr>, name: &str) -> bool {
    search_path.is_some_and(|path| {
        env::split_paths(path).any(|dir| {
            fs::metadata(dir.join(name)).is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
    })
}

fn ding_installed() -> bool {
    let data_dirs = env::var_os("XDG_DATA_DIRS")
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    data_home()
        .into_iter()
        .chain(env::split_paths(&data_dirs))
        .filter(|dir| dir.is_absolute())
        .any(|dir| dir.join("gnome-shell/extensions/ding@rastersoft.com/metadata.json").is_file())
}

const SCRIPT_ENTRIES: [ScriptEntry; 6] = [
    ScriptEntry::defang("defang", "Defang", HashAlgorithm::Sha256),
    ScriptEntry::defang("defang-md5", "Defang MD5", HashAlgorithm::Md5),
    ScriptEntry::defang("defang-sha1", "Defang SHA-1", HashAlgorithm::Sha1),
    ScriptEntry::defang("defang-sha512", "Defang SHA-512", HashAlgorithm::Sha512),
    ScriptEntry::defang("defang-blake3", "Defang BLAKE3", HashAlgorithm::Blake3),
    ScriptEntry::refang(),
];

pub(crate) fn rename_file(path: &Path, operation: RenameOperation) -> io::Result<PathBuf> {
    match operation {
        RenameOperation::Defang(algorithm) => {
            let new_path = rename::defang_path(path, algorithm, " ")?;
            strip_execute_bits(&new_path)?;
            Ok(new_path)
        }
        RenameOperation::Refang(algorithm) => {
            let new_path = rename::refang_path(path, algorithm, " ")?;
            add_execute_bits(&new_path)?;
            Ok(new_path)
        }
    }
}

pub(crate) fn install_context_menu(target: Option<FileManager>, exe_path: &Path) -> io::Result<()> {
    let exe_path = exe_path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "MalFang_tool path is not UTF-8")
    })?;
    let search_path = env::var_os("PATH");
    let mut installed = HashSet::new();
    let mut refresh_xfce = false;

    for &manager in managers(&target) {
        let registration = manager.registration_path()?;
        if target.is_none() && !manager.detected(&registration, search_path.as_deref()) {
            continue;
        }

        if !installed.contains(&registration) {
            let wrappers = manager.wrappers_dir(&registration)?;
            for entry in SCRIPT_ENTRIES {
                let path = wrappers.join(manager.wrapper_name(entry));
                let script = context_menu_script(exe_path, manager, entry);
                install_managed_file(&path, &script, 0o755)?;
            }
            if !manager.uses_scripts_menu() {
                actions::install(manager, &registration, &wrappers)?;
            }
            println!("  {}: {}", manager.name(), registration.display());
            installed.insert(registration);
            refresh_xfce |= matches!(manager, FileManager::Thunar);
        }
        if let Some(hint) = manager.refresh_hint() {
            println!("    {hint}");
        }
    }

    if installed.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No supported file manager detected; see --help for --file-manager options",
        ));
    }
    if refresh_xfce {
        refresh::offer_xfce_refresh(search_path.as_deref());
    }
    Ok(())
}

pub(crate) fn uninstall_context_menu(target: Option<FileManager>) -> io::Result<()> {
    let mut checked = HashSet::new();
    let mut total_removed = 0;
    let mut refresh_xfce = false;
    for &manager in managers(&target) {
        let registration = manager.registration_path()?;
        if !checked.insert(registration.clone()) {
            println!("{}: already checked shared path {}", manager.name(), registration.display());
            continue;
        }
        println!("{}: checking {}", manager.name(), registration.display());

        let mut removed = match manager {
            FileManager::Thunar => {
                let count = actions::update_thunar(&registration, None)?;
                if count > 0 {
                    println!(
                        "  Removed {count} MalFang action(s) from {}; kept the configuration file.",
                        registration.display()
                    );
                    refresh_xfce = true;
                }
                count
            }
            FileManager::Dolphin | FileManager::Pcmanfm => actions::uninstall(manager, &registration)?,
            _ => 0,
        };
        let wrappers = manager.wrappers_dir(&registration)?;
        for entry in SCRIPT_ENTRIES {
            removed += remove_managed_file(&wrappers.join(manager.wrapper_name(entry)))?;
        }
        if removed == 0 {
            println!("  No MalFang entries removed.");
        }
        total_removed += removed;
    }
    println!("Context-menu uninstall complete: removed {total_removed} managed file(s) or action(s).");
    if refresh_xfce {
        refresh::offer_xfce_refresh(env::var_os("PATH").as_deref());
    }
    Ok(())
}

fn managers(target: &Option<FileManager>) -> &[FileManager] {
    match target {
        Some(manager) => std::slice::from_ref(manager),
        None => &FileManager::ALL,
    }
}

fn xdg_home(var: &str, fallback: &str) -> io::Result<PathBuf> {
    match env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()) {
        Some(path) => Ok(path),
        None => Ok(home_dir()?.join(fallback)),
    }
}

fn data_home() -> io::Result<PathBuf> {
    xdg_home("XDG_DATA_HOME", ".local/share")
}

fn config_home() -> io::Result<PathBuf> {
    xdg_home("XDG_CONFIG_HOME", ".config")
}

#[derive(Clone, Copy)]
struct ScriptEntry {
    id: &'static str,
    file_name: &'static str,
    operation: RenameOperation,
}

impl ScriptEntry {
    const fn defang(id: &'static str, file_name: &'static str, algorithm: HashAlgorithm) -> Self {
        Self {
            id,
            file_name,
            operation: RenameOperation::Defang(algorithm),
        }
    }

    const fn refang() -> Self {
        Self {
            id: "refang",
            file_name: "Refang",
            operation: RenameOperation::Refang(HashAlgorithm::Sha256),
        }
    }
}

fn strip_execute_bits(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let new_mode = mode & !0o111;
    if new_mode != mode {
        permissions.set_mode(new_mode);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn add_execute_bits(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let new_mode = mode | 0o111;
    if new_mode != mode {
        permissions.set_mode(new_mode);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn context_menu_script(exe_path: &str, manager: FileManager, entry: ScriptEntry) -> String {
    let (command_args, success_message) = match entry.operation {
        RenameOperation::Defang(algorithm) => (
            format!("--hash {}", algorithm.cli_name()),
            format!("Defanged $count item(s) with {}.", algorithm.label()),
        ),
        RenameOperation::Refang(algorithm) => (
            format!("--refang --hash {}", algorithm.cli_name()),
            "Refanged $count item(s).".to_string(),
        ),
    };
    let selection_fallback = manager.selection_var().map_or_else(String::new, |var| {
        format!(
            r#"# Prefer argv: the environment fallback cannot represent embedded newlines.
if [ "$#" -eq 0 ]; then
    old_ifs=$IFS
    IFS='
'
    # Split the newline-delimited selection without glob expansion.
    set -- ${{{var}:-}}
    IFS=$old_ifs
fi

"#
        )
    });
    format!(
        include_str!("wrapper.sh.in"),
        selection_fallback = selection_fallback,
        exe_path = shell_single_quote(exe_path),
        command_args = command_args,
        success_message = success_message,
    )
}

const MANAGED_MARKER: &str = "# Managed by MalFang_tool\n";

fn install_managed_file(path: &Path, contents: &str, mode: u32) -> io::Result<()> {
    if matches!(managed_file_state(path)?, ManagedFile::Unrelated) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Refusing to replace an unrelated file: {}", path.display()),
        ));
    }
    atomic_write(path, contents, mode)
}

fn remove_managed_file(path: &Path) -> io::Result<usize> {
    let with_path = |err: io::Error| io::Error::new(err.kind(), format!("{}: {err}", path.display()));
    match managed_file_state(path).map_err(with_path)? {
        ManagedFile::Owned => match fs::remove_file(path) {
            Ok(()) => {
                println!("  Removed {}", path.display());
                Ok(1)
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(err) => Err(with_path(err)),
        },
        ManagedFile::Unrelated => {
            println!("  Kept {} (not recognized as MalFang-managed)", path.display());
            Ok(0)
        }
        ManagedFile::Missing => Ok(0),
    }
}

fn atomic_write(path: &Path, contents: &str, mode: u32) -> io::Result<()> {
    let parent = path.parent().expect("context menu path has a parent");
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(contents.as_bytes())?;
    temp.as_file().set_permissions(fs::Permissions::from_mode(mode))?;
    temp.persist(path)?;
    Ok(())
}

enum ManagedFile {
    Missing,
    Owned,
    Unrelated,
}

fn managed_file_state(path: &Path) -> io::Result<ManagedFile> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => return Ok(ManagedFile::Unrelated),
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(ManagedFile::Missing),
        Err(err) => return Err(err),
    }

    let contents = fs::read(path)?;
    let Ok(contents) = std::str::from_utf8(&contents) else {
        return Ok(ManagedFile::Unrelated);
    };
    let body = contents.strip_prefix("#!/bin/sh\n").unwrap_or(contents);
    // Older releases installed the same unmarked wrapper for the three GNOME-derived managers.
    let legacy = contents.starts_with("#!/bin/sh\nset -eu\n")
        && FileManager::ALL
            .iter()
            .filter_map(|manager| manager.selection_var())
            .all(|var| contents.contains(var))
        && contents.contains(
            r#"notify-send "MalFang_tool" "Processed $count item(s); $failed failed.""#,
        );
    if body.starts_with(MANAGED_MARKER) || legacy {
        Ok(ManagedFile::Owned)
    } else {
        Ok(ManagedFile::Unrelated)
    }
}

fn home_dir() -> io::Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "HOME is not set to an absolute path; cannot locate Linux context-menu scripts",
            )
        })
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
