mod install;
mod platform;
mod rename;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Parser};
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
    if args.install_context_menu {
        let executable = install::context_menu_executable(args.install_binary)?;
        #[cfg(target_os = "linux")]
        platform::install_context_menu(args.file_manager, &executable)?;
        #[cfg(target_os = "windows")]
        platform::install_context_menu(&executable)?;
        println!("Context menu installed.");
        return Ok(());
    }

    if args.uninstall_context_menu {
        #[cfg(target_os = "linux")]
        return platform::uninstall_context_menu(args.file_manager);
        #[cfg(target_os = "windows")]
        return platform::uninstall_context_menu();
    }

    let path = args
        .path
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing path argument"))?;
    let operation = if args.refang {
        RenameOperation::Refang(args.algorithm)
    } else {
        RenameOperation::Defang(args.algorithm)
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

#[derive(Debug, Parser)]
#[command(
    name = "MalFang_tool",
    about = "Defang files with trailing hashes or refang them with leading hashes",
    arg_required_else_help = true
)]
struct Args {
    #[arg(short = 'a', long = "hash", value_enum, default_value_t = HashAlgorithm::Sha256)]
    algorithm: HashAlgorithm,

    #[arg(
        long = "refang",
        action = ArgAction::SetTrue,
        conflicts_with_all = ["install_context_menu", "uninstall_context_menu"]
    )]
    refang: bool,

    #[arg(
        value_name = "PATH",
        conflicts_with_all = ["install_context_menu", "uninstall_context_menu"],
        required_unless_present_any = ["install_context_menu", "uninstall_context_menu"]
    )]
    path: Option<PathBuf>,

    #[arg(long, action = ArgAction::SetTrue, group = "context_menu")]
    install_context_menu: bool,

    /// Copy the executable to a stable per-user location and register menus there.
    #[arg(
        long,
        requires = "install_context_menu",
        conflicts_with = "uninstall_context_menu"
    )]
    install_binary: bool,

    #[arg(long, action = ArgAction::SetTrue, group = "context_menu")]
    uninstall_context_menu: bool,

    /// Target a Linux file manager or desktop (aliases: xfdesktop, plasma, nemo-desktop).
    #[cfg(target_os = "linux")]
    #[arg(long, value_enum, requires = "context_menu")]
    file_manager: Option<platform::FileManager>,
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn binary_install_requires_menu_install_and_conflicts_with_uninstall() {
        // Parse only: invalid combinations must never run registry operations.
        for args in [
            vec!["MalFang_tool", "--install-binary"],
            vec![
                "MalFang_tool",
                "--install-binary",
                "--uninstall-context-menu",
            ],
        ] {
            assert!(Args::try_parse_from(args).is_err());
        }
        assert!(Args::try_parse_from([
            "MalFang_tool",
            "--install-binary",
            "--install-context-menu",
        ])
        .is_ok());
    }
}
