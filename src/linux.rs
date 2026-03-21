use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const SCRIPT_DIRS: [&str; 3] = [
    ".local/share/nautilus/scripts",
    ".local/share/nemo/scripts",
    ".config/caja/scripts",
];

const SCRIPT_ENTRIES: [(&str, crate::HashAlgorithm, &str); 4] = [
    ("Defang", crate::HashAlgorithm::Sha256, "SHA-256"),
    ("Defang MD5", crate::HashAlgorithm::Md5, "MD5"),
    ("Defang SHA-1", crate::HashAlgorithm::Sha1, "SHA-1"),
    ("Defang SHA-512", crate::HashAlgorithm::Sha512, "SHA-512"),
];

const BLAKE3_SCRIPT_ENTRY: (&str, crate::HashAlgorithm, &str) =
    ("Defang BLAKE3", crate::HashAlgorithm::Blake3, "BLAKE3");

pub(crate) fn defang_file(path: &Path, algorithm: crate::HashAlgorithm) -> io::Result<PathBuf> {
    let hash = crate::compute_hash_hex(path, algorithm)?;
    let new_path = crate::rename_with_hash(path, &hash, " ")?;
    strip_execute_bits(&new_path)?;
    Ok(new_path)
}

pub(crate) fn install_context_menu() -> io::Result<()> {
    let exe_path = env::current_exe()?;
    let exe_path = shell_single_quote(&exe_path.to_string_lossy());
    let home_dir = home_dir()?;

    for relative_dir in SCRIPT_DIRS {
        let script_dir = home_dir.join(relative_dir);
        fs::create_dir_all(&script_dir)?;

        for (file_name, algorithm, label) in script_entries() {
            write_context_menu_script(&script_dir.join(file_name), &exe_path, algorithm, label)?;
        }
    }

    Ok(())
}

pub(crate) fn uninstall_context_menu() -> io::Result<()> {
    let home_dir = home_dir()?;

    for relative_dir in SCRIPT_DIRS {
        let script_dir = home_dir.join(relative_dir);

        for (file_name, _, _) in script_entries() {
            let script_path = script_dir.join(file_name);
            match fs::remove_file(&script_path) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }

    Ok(())
}

fn script_entries() -> impl Iterator<Item = (&'static str, crate::HashAlgorithm, &'static str)> {
    SCRIPT_ENTRIES
        .into_iter()
        .chain(std::iter::once(BLAKE3_SCRIPT_ENTRY))
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

fn write_context_menu_script(
    script_path: &Path,
    exe_path: &str,
    algorithm: crate::HashAlgorithm,
    label: &str,
) -> io::Result<()> {
    let script = format!(
        "#!/bin/sh\n\
set -eu\n\
\n\
if [ \"$#\" -eq 0 ]; then\n\
    if [ -n \"${{NAUTILUS_SCRIPT_SELECTED_FILE_PATHS:-}}\" ]; then\n\
        OLD_IFS=$IFS\n\
        IFS='\n\
'\n\
        set -- $NAUTILUS_SCRIPT_SELECTED_FILE_PATHS\n\
        IFS=$OLD_IFS\n\
    elif [ -n \"${{NEMO_SCRIPT_SELECTED_FILE_PATHS:-}}\" ]; then\n\
        OLD_IFS=$IFS\n\
        IFS='\n\
'\n\
        set -- $NEMO_SCRIPT_SELECTED_FILE_PATHS\n\
        IFS=$OLD_IFS\n\
    elif [ -n \"${{CAJA_SCRIPT_SELECTED_FILE_PATHS:-}}\" ]; then\n\
        OLD_IFS=$IFS\n\
        IFS='\n\
'\n\
        set -- $CAJA_SCRIPT_SELECTED_FILE_PATHS\n\
        IFS=$OLD_IFS\n\
    fi\n\
fi\n\
\n\
if [ \"$#\" -eq 0 ]; then\n\
    exit 0\n\
fi\n\
\n\
count=0\n\
failed=0\n\
for item in \"$@\"; do\n\
    if {exe_path} --hash {} \"$item\"; then\n\
        count=$((count + 1))\n\
    else\n\
        failed=$((failed + 1))\n\
    fi\n\
done\n\
\n\
if command -v notify-send >/dev/null 2>&1; then\n\
    if [ \"$failed\" -eq 0 ]; then\n\
        notify-send \"HashRename\" \"Defanged $count item(s) with {label}.\"\n\
    else\n\
        notify-send \"HashRename\" \"Defanged $count item(s); $failed failed.\"\n\
    fi\n\
fi\n\
\n\
[ \"$failed\" -eq 0 ]\n",
        algorithm.cli_name()
    );

    fs::write(script_path, script)?;
    let mut permissions = fs::metadata(script_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script_path, permissions)
}

fn home_dir() -> io::Result<PathBuf> {
    match env::var_os("HOME") {
        Some(home) => Ok(PathBuf::from(home)),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "HOME is not set; cannot install Linux context-menu scripts",
        )),
    }
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
