use std::{env, fs, io, path::PathBuf};

/// Copy the executable to a stable, per-user location when explicitly requested.
pub(crate) fn context_menu_executable(install_binary: bool) -> io::Result<PathBuf> {
    let source = env::current_exe()?;
    if !install_binary {
        return Ok(source);
    }

    #[cfg(target_os = "linux")]
    let (variable, relative) = ("HOME", ".local/bin/MalFang_tool");
    #[cfg(target_os = "windows")]
    let (variable, relative) = ("LOCALAPPDATA", "Programs/MalFang_tool/MalFang_tool.exe");

    let base = env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{variable} must be set to an absolute path to install the executable"),
            )
        })?;
    let destination = base.join(relative);
    if destination.exists() && fs::canonicalize(&source)? == fs::canonicalize(&destination)? {
        println!("Executable already installed: {}", destination.display());
        return Ok(destination);
    }
    fs::create_dir_all(
        destination
            .parent()
            .expect("installation path has a parent"),
    )?;
    fs::copy(&source, &destination).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "Could not install executable to {}: {err}",
                destination.display()
            ),
        )
    })?;
    println!("Installed executable: {}", destination.display());
    Ok(destination)
}
