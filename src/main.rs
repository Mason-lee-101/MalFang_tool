mod config;
mod install;
mod platform;
mod rename;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Parser, ValueEnum};
use rename::{HashAlgorithm, RenameOperation};

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("This tool currently supports only Linux and Windows.");

fn main() {
    let args = Args::parse();

    if let Err(err) = run(args) {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> io::Result<()> {
    if let Some(algorithm) = args.default_hash {
        let path = config::set_default_hash(algorithm)?;
        println!(
            "Default hash set to {}: {}",
            algorithm.cli_name(),
            path.display()
        );
        return Ok(());
    }

    if let Some(scope) = args.install {
        let executable = install::context_menu_executable(true)?;
        if scope.includes_path() {
            install::add_to_user_path(&executable)?;
        }
        if scope.includes_menus() {
            #[cfg(target_os = "linux")]
            platform::install_context_menu(args.file_manager, &executable)?;
            #[cfg(target_os = "windows")]
            platform::install_context_menu(&executable)?;
            println!("Context menu installed.");
        }
        return Ok(());
    }

    if let Some(scope) = args.uninstall {
        if scope.includes_menus() {
            #[cfg(target_os = "linux")]
            platform::uninstall_context_menu(args.file_manager)?;
            #[cfg(target_os = "windows")]
            platform::uninstall_context_menu()?;
        }
        if scope.includes_path() {
            install::uninstall()?;
        }
        return Ok(());
    }

    let path = args
        .path
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing path argument"))?;
    let algorithm = match args.algorithm {
        Some(algorithm) => algorithm,
        None => config::default_hash()?,
    };
    let operation = if args.refang {
        RenameOperation::Refang(algorithm)
    } else {
        RenameOperation::Defang(algorithm)
    };

    if path.is_file() {
        let renamed_path = platform::rename_file(path, operation)?;
        println!("{}: {}", operation.label_past_tense(), path.display());
        println!("Renamed to: {}", renamed_path.display());
        return Ok(());
    }

    if path.is_dir() {
        let renamed_files = rename_folder_items(path, operation)?;
        println!(
            "{} {} item(s) in folder: {}",
            operation.label_past_tense(),
            renamed_files.len(),
            path.display()
        );
        for (old_path, new_path) in renamed_files {
            println!("  {} -> {}", old_path.display(), new_path.display());
        }
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Path must point to an existing file or folder",
    ))
}

fn rename_folder_items(
    folder_path: &Path,
    operation: RenameOperation,
) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    let mut renamed_files = Vec::new();

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if !entry_path.is_file() {
            continue;
        }

        let old_path = entry_path.clone();
        let new_path = platform::rename_file(&entry_path, operation)?;
        renamed_files.push((old_path, new_path));
    }

    Ok(renamed_files)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum InstallScope {
    All,
    Path,
    Menus,
}

impl InstallScope {
    fn includes_path(self) -> bool {
        matches!(self, Self::All | Self::Path)
    }

    fn includes_menus(self) -> bool {
        matches!(self, Self::All | Self::Menus)
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "MalFang_tool",
    about = "Defang files with trailing hashes or refang them with leading hashes",
    after_help = "Examples:\n  Defang: MalFang_tool sample.exe\n          Renames to sample.exe_<hash>\n\n  Refang: MalFang_tool --refang \"sample.exe_<hash>\"\n          Renames to <hash>_sample.exe",
    arg_required_else_help = true
)]
struct Args {
    /// Override the saved default for this command (fallback: sha256).
    #[arg(short = 'a', long = "hash", value_enum)]
    algorithm: Option<HashAlgorithm>,

    /// Save the default algorithm for CLI and default menu actions.
    #[arg(long, value_enum, value_name = "ALGORITHM", conflicts_with_all = ["algorithm", "refang", "path", "install", "uninstall"])]
    default_hash: Option<HashAlgorithm>,

    #[arg(
        long = "refang",
        action = ArgAction::SetTrue,
        conflicts_with_all = ["install", "uninstall"]
    )]
    refang: bool,

    #[arg(
        value_name = "PATH",
        conflicts_with_all = ["install", "uninstall"],
        required_unless_present_any = ["install", "uninstall", "default_hash"]
    )]
    path: Option<PathBuf>,

    /// Install everything (default), binary + user PATH, or menus + binary without PATH.
    #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "all", value_name = "SCOPE", group = "installation")]
    install: Option<InstallScope>,

    /// Remove everything (default), binary + user PATH, or only menus.
    #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "all", value_name = "SCOPE", group = "installation")]
    uninstall: Option<InstallScope>,

    /// Target a Linux file manager or desktop.
    #[cfg(target_os = "linux")]
    #[arg(
        long,
        value_enum,
        requires = "installation",
        help = "Target a Linux file manager or desktop.\nDesktop targets: xfdesktop (Xfce), plasma (KDE), ding (GNOME Desktop Icons NG),\n                 nemo-desktop (Cinnamon), caja (MATE)."
    )]
    file_manager: Option<platform::FileManager>,
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn saved_default_command_is_standalone() {
        let args = Args::try_parse_from(["MalFang_tool", "--default-hash", "blake3"]).unwrap();
        assert_eq!(args.default_hash, Some(HashAlgorithm::Blake3));
        assert!(args.algorithm.is_none());
        assert!(Args::try_parse_from(["MalFang_tool", "sample.exe"])
            .unwrap()
            .algorithm
            .is_none());
        for other in ["--install", "--uninstall", "--refang", "sample.exe"] {
            assert!(
                Args::try_parse_from(["MalFang_tool", "--default-hash", "md5", other]).is_err()
            );
        }
        assert!(Args::try_parse_from(["MalFang_tool", "--default-hash", "bad"]).is_err());
        assert!(Args::try_parse_from([
            "MalFang_tool",
            "--default-hash",
            "md5",
            "--hash",
            "sha256"
        ])
        .is_err());
    }

    #[test]
    fn installation_scopes_and_defaults() {
        for operation in ["--install", "--uninstall"] {
            let args = Args::try_parse_from(["MalFang_tool", operation]).unwrap();
            assert_eq!(args.install.or(args.uninstall), Some(InstallScope::All));
            for (value, scope) in [
                ("all", InstallScope::All),
                ("path", InstallScope::Path),
                ("menus", InstallScope::Menus),
            ] {
                let args = Args::try_parse_from(["MalFang_tool", operation, value]).unwrap();
                assert_eq!(args.install.or(args.uninstall), Some(scope));
            }
            for other in ["invalid", "sample.exe", "--refang"] {
                assert!(Args::try_parse_from(["MalFang_tool", operation, other]).is_err());
            }
        }
        assert!(Args::try_parse_from(["MalFang_tool", "--install", "--uninstall"]).is_err());
        assert!(Args::try_parse_from(["MalFang_tool", "--install=path"]).is_ok());
        assert!(InstallScope::All.includes_path() && InstallScope::All.includes_menus());
        assert!(InstallScope::Path.includes_path() && !InstallScope::Path.includes_menus());
        assert!(!InstallScope::Menus.includes_path() && InstallScope::Menus.includes_menus());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn help_lists_linux_desktop_targets() {
        use clap::CommandFactory;
        for help in [
            Args::command().render_help(),
            Args::command().render_long_help(),
        ] {
            let text = help.to_string();
            for target in [
                "xfdesktop (Xfce)",
                "plasma (KDE)",
                "ding (GNOME Desktop Icons NG)",
                "nemo-desktop (Cinnamon)",
                "caja (MATE)",
            ] {
                assert!(text.contains(target), "Missing desktop target: {target}");
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn file_manager_accepts_scoped_menu_operations() {
        for operation in ["--install", "--uninstall"] {
            assert!(Args::try_parse_from([
                "MalFang_tool",
                operation,
                "menus",
                "--file-manager",
                "nemo"
            ])
            .is_ok());
        }
        assert!(
            Args::try_parse_from(["MalFang_tool", "--file-manager", "nemo", "sample.exe"]).is_err()
        );
    }
}
