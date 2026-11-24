use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

/// Persist the installed directory for future shells, without changing system PATH.
pub(crate) fn add_to_user_path(executable: &Path) -> io::Result<()> {
    let directory = executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Executable has no parent directory",
        )
    })?;
    persist_user_path(directory, false)?;
    println!(
        "User PATH configured: {} (open a new terminal to use it)",
        directory.display()
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn persist_user_path(directory: &Path, remove: bool) -> io::Result<()> {
    use std::io::Write;
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME must be an absolute path"))?;
    let quoted = format!("'{}'", directory.to_string_lossy().replace('\'', "'\\''"));
    let entry = format!(
        "# MalFang_tool user PATH\ncase \":$PATH:\" in\n    *:{quoted}:*) ;;\n    *) export PATH=\"$PATH\":{quoted} ;;\nesac\n"
    );
    for name in [".profile", ".bashrc", ".zshrc"] {
        let path = home.join(name);
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err),
        };
        if remove {
            if content.contains(&entry) {
                fs::write(path, content.replace(&entry, ""))?;
            }
        } else if !content.contains(&entry) {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            if !content.is_empty() && !content.ends_with('\n') {
                writeln!(file)?;
            }
            file.write_all(entry.as_bytes())?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn persist_user_path(directory: &Path, remove: bool) -> io::Result<()> {
    // Read the unexpanded registry value so existing %VARIABLE% references survive.
    let script = r#"
$ErrorActionPreference = 'Stop'
$key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
try {
    $old = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    $directory = $env:MALFANG_INSTALL_DIRECTORY
    $found = $false
    $remaining = @()
    foreach ($entry in ($old -split ';')) {
        if ([Environment]::ExpandEnvironmentVariables($entry).TrimEnd('\') -ieq $directory.TrimEnd('\')) { $found = $true }
        else { $remaining += $entry }
    }
    $remove = $env:MALFANG_REMOVE_PATH -eq '1'
    if (($remove -and $found) -or (!$remove -and !$found)) {
        $value = if ($remove) { $remaining -join ';' } elseif ($old) { $old.TrimEnd(';') + ';' + $directory } else { $directory }
        $key.SetValue('Path', $value, [Microsoft.Win32.RegistryValueKind]::ExpandString)
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public class MalFangEnvironment {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint msg, UIntPtr wparam, string lparam, uint flags, uint timeout, out UIntPtr result);
}
'@
        $result = [UIntPtr]::Zero
        [void][MalFangEnvironment]::SendMessageTimeout([IntPtr]0xffff, 0x1a, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result)
    }
} finally { $key.Dispose() }
"#;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("MALFANG_INSTALL_DIRECTORY", directory)
        .env("MALFANG_REMOVE_PATH", if remove { "1" } else { "0" })
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Could not update user PATH: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

/// Copy the executable to a stable, per-user location when explicitly requested.
pub(crate) fn context_menu_executable(install_binary: bool) -> io::Result<PathBuf> {
    let source = env::current_exe()?;
    if !install_binary {
        return Ok(source);
    }

    let destination = installed_executable()?;
    if destination.exists() && fs::canonicalize(&source)? == fs::canonicalize(&destination)? {
        println!("Executable already installed: {}", destination.display());
        return Ok(destination);
    }
    fs::create_dir_all(destination.parent().expect("installation path has a parent"))?;
    fs::copy(&source, &destination).map_err(|err| {
        io::Error::new(err.kind(), format!("Could not install executable to {}: {err}", destination.display()))
    })?;
    println!("Installed executable: {}", destination.display());
    Ok(destination)
}

fn installed_executable() -> io::Result<PathBuf> {
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
    Ok(base.join(relative))
}

/// Remove only our installed executable and PATH configuration, never its shared directory.
pub(crate) fn uninstall() -> io::Result<()> {
    let destination = installed_executable()?;
    persist_user_path(destination.parent().expect("installation path has a parent"), true)?;
    if destination.exists() {
        #[cfg(target_os = "linux")]
        fs::remove_file(&destination)?;
        #[cfg(target_os = "windows")]
        {
            if fs::canonicalize(env::current_exe()?)? == fs::canonicalize(&destination)? {
                use std::os::windows::process::CommandExt;
                use std::process::{Command, Stdio};
                // A separate process waits for this executable to exit before deleting it.
                Command::new("powershell.exe")
                    .args(["-NoProfile", "-NonInteractive", "-Command", r#"
Wait-Process -Id $env:MALFANG_UNINSTALL_PID -ErrorAction SilentlyContinue
for ($i = 0; $i -lt 20; $i++) {
    try { Remove-Item -LiteralPath $env:MALFANG_UNINSTALL_FILE -Force -ErrorAction Stop; exit 0 }
    catch { Start-Sleep -Milliseconds 250 }
}
exit 1
"#])
                    .env("MALFANG_UNINSTALL_PID", std::process::id().to_string())
                    .env("MALFANG_UNINSTALL_FILE", &destination)
                    .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                    .creation_flags(0x08000000)
                    .spawn()?;
                println!("Installed executable scheduled for deletion after exit.");
            } else {
                fs::remove_file(&destination)?;
            }
        }
    }
    println!("Uninstalled. Open a new terminal to refresh PATH.");
    Ok(())
}
