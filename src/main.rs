#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Parser, ValueEnum};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

#[cfg(target_os = "linux")]
use crate::linux as platform;
#[cfg(target_os = "windows")]
use crate::windows as platform;

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
        platform::install_context_menu()?;
        println!("Context menu installed.");
        return Ok(());
    }

    if args.uninstall_context_menu {
        platform::uninstall_context_menu()?;
        println!("Context menu uninstalled.");
        return Ok(());
    }

    let path = args
        .path
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing path argument"))?;
    if path.is_file() {
        let renamed_path = platform::defang_file(path, args.algorithm)?;
        println!("Defanged: {}", path.display());
        println!("Renamed to: {}", renamed_path.display());
        return Ok(());
    }

    if path.is_dir() {
        let renamed_files = defang_folder_items(path, args.algorithm)?;
        println!(
            "Defanged {} item(s) in folder: {}",
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

fn defang_folder_items(
    folder_path: &Path,
    algorithm: HashAlgorithm,
) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    let mut renamed_files = Vec::new();

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if !entry_path.is_file() {
            continue;
        }

        let old_path = entry_path.clone();
        let new_path = platform::defang_file(&entry_path, algorithm)?;
        renamed_files.push((old_path, new_path));
    }

    Ok(renamed_files)
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum HashAlgorithm {
    Md5,
    #[value(alias = "sha-1")]
    Sha1,
    #[value(alias = "sha-256")]
    Sha256,
    #[value(alias = "sha-512")]
    Sha512,
    Blake3,
}

impl HashAlgorithm {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Md5 => "MD5",
            Self::Sha1 => "SHA-1",
            Self::Sha256 => "SHA-256",
            Self::Sha512 => "SHA-512",
            Self::Blake3 => "BLAKE3",
        }
    }

    pub(crate) fn cli_name(&self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
            Self::Blake3 => "blake3",
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "hashrename",
    about = "Defang a file by adding a hash to the filename",
    arg_required_else_help = true
)]
struct Args {
    #[arg(short = 'a', long = "hash", value_enum, default_value_t = HashAlgorithm::Sha256)]
    algorithm: HashAlgorithm,

    #[arg(
        value_name = "PATH",
        conflicts_with_all = ["install_context_menu", "uninstall_context_menu"],
        required_unless_present_any = ["install_context_menu", "uninstall_context_menu"]
    )]
    path: Option<PathBuf>,

    #[arg(long = "install-context-menu", action = ArgAction::SetTrue, conflicts_with = "uninstall_context_menu")]
    install_context_menu: bool,

    #[arg(long = "uninstall-context-menu", action = ArgAction::SetTrue)]
    uninstall_context_menu: bool,
}

pub(crate) fn compute_hash_hex(path: &Path, algorithm: HashAlgorithm) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let hash = match algorithm {
        HashAlgorithm::Md5 => {
            let mut hasher = Md5::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::Sha1 => {
            let mut hasher = Sha1::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::Blake3 => blake3::hash(&bytes).to_hex().to_string(),
    };
    Ok(hash)
}

pub(crate) fn rename_with_hash(path: &Path, hash: &str, separator: &str) -> io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Input path has no file name"))?
        .to_string_lossy();

    let suffix = format!("{separator}{hash}");
    if file_name
        .to_ascii_lowercase()
        .ends_with(&suffix.to_ascii_lowercase())
    {
        return Ok(path.to_path_buf());
    }

    let new_file_name = format!("{file_name}{separator}{hash}");

    let new_path = parent.join(new_file_name);
    fs::rename(path, &new_path)?;
    Ok(new_path)
}
