use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const SHELL_ROOTS: [(&str, &str); 2] = [
    (r"HKCU\Software\Classes\*\shell", "file"),
    (r"HKCU\Software\Classes\Folder\shell", "folder"),
];
const MENU_DEFAULT_KEY_NAME: &str = "HashRenameDefang";
const MENU_OTHER_KEY_NAME: &str = "HashRenameDefangOther";

pub(crate) fn defang_file(path: &Path, algorithm: crate::HashAlgorithm) -> io::Result<PathBuf> {
    let hash = crate::compute_hash_hex(path, algorithm)?;
    crate::rename_with_hash(path, &hash, " ")
}

pub(crate) fn install_context_menu() -> io::Result<()> {
    let exe_path = std::env::current_exe()?;
    let exe_path_display = exe_path.display().to_string();
    let default_command_value = format!(
        "\"{}\" --hash {} \"%1\"",
        exe_path_display,
        crate::HashAlgorithm::Sha256.cli_name()
    );

    for (shell_root, target_label) in SHELL_ROOTS {
        install_context_menu_for_root(
            shell_root,
            target_label,
            &exe_path_display,
            &default_command_value,
        )?;
    }

    Ok(())
}

pub(crate) fn uninstall_context_menu() -> io::Result<()> {
    for shell_root in [
        r"HKCU\Software\Classes\*\shell",
        r"HKCU\Software\Classes\SystemFileAssociations\*\shell",
        r"HKCU\Software\Classes\Folder\shell",
    ] {
        let context_menu_default_key = format!(r"{shell_root}\{MENU_DEFAULT_KEY_NAME}");
        let context_menu_other_key = format!(r"{shell_root}\{MENU_OTHER_KEY_NAME}");
        delete_reg_key_if_exists(&context_menu_other_key)?;
        delete_reg_key_if_exists(&context_menu_default_key)?;
    }

    Ok(())
}

fn install_context_menu_for_root(
    shell_root: &str,
    target_label: &str,
    exe_path: &str,
    default_command_value: &str,
) -> io::Result<()> {
    let context_menu_default_key = format!(r"{shell_root}\{MENU_DEFAULT_KEY_NAME}");
    let context_menu_default_command_key = format!(r"{context_menu_default_key}\command");
    let context_menu_other_key = format!(r"{shell_root}\{MENU_OTHER_KEY_NAME}");

    delete_reg_key_if_exists(&context_menu_other_key)?;
    delete_reg_key_if_exists(&context_menu_default_key)?;

    add_reg_default_value(
        &context_menu_default_key,
        &format!("Defang {target_label}"),
        &format!("create the default {target_label} context menu label"),
    )?;
    add_reg_named_value(
        &context_menu_default_key,
        "Icon",
        exe_path,
        &format!("set the icon for the default {target_label} context menu"),
    )?;
    add_reg_default_value(
        &context_menu_default_command_key,
        default_command_value,
        &format!("set the command for the default {target_label} context menu"),
    )?;

    add_reg_named_value(
        &context_menu_other_key,
        "MUIVerb",
        &format!("Defang {target_label} with other hashes"),
        &format!("create the other-hashes {target_label} submenu label"),
    )?;
    add_reg_named_value(
        &context_menu_other_key,
        "SubCommands",
        "",
        &format!("mark the other-hashes {target_label} entry as a submenu"),
    )?;
    add_reg_named_value(
        &context_menu_other_key,
        "Icon",
        exe_path,
        &format!("set the icon for the other-hashes {target_label} submenu"),
    )?;

    for algorithm in [
        crate::HashAlgorithm::Md5,
        crate::HashAlgorithm::Sha1,
        crate::HashAlgorithm::Sha512,
        crate::HashAlgorithm::Blake3,
    ] {
        let sub_key = format!(r"{context_menu_other_key}\shell\{}", algorithm.cli_name());
        let sub_command_key = format!(r"{sub_key}\command");
        let command_value = format!("\"{}\" --hash {} \"%1\"", exe_path, algorithm.cli_name());
        let menu_text = format!("Defang {target_label} ({})", algorithm.label());

        add_reg_default_value(
            &sub_key,
            &menu_text,
            &format!(
                "create the {} submenu item for {target_label}",
                algorithm.cli_name()
            ),
        )?;
        add_reg_default_value(
            &sub_command_key,
            &command_value,
            &format!(
                "set the command for the {} submenu item for {target_label}",
                algorithm.cli_name()
            ),
        )?;
    }

    Ok(())
}

fn add_reg_default_value(key: &str, data: &str, context: &str) -> io::Result<()> {
    run_reg_command(
        &[
            "add".to_string(),
            key.to_string(),
            "/ve".to_string(),
            "/d".to_string(),
            data.to_string(),
            "/f".to_string(),
        ],
        context,
    )
}

fn add_reg_named_value(key: &str, value_name: &str, data: &str, context: &str) -> io::Result<()> {
    run_reg_command(
        &[
            "add".to_string(),
            key.to_string(),
            "/v".to_string(),
            value_name.to_string(),
            "/d".to_string(),
            data.to_string(),
            "/f".to_string(),
        ],
        context,
    )
}

fn run_reg_command(args: &[String], context: &str) -> io::Result<()> {
    let output = Command::new("reg").args(args).output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let command = format!("reg {}", args.join(" "));
        let mut message = format!(
            "Failed to {context}. Command: {command}. Exit status: {}.",
            output.status
        );
        if !stdout.is_empty() {
            message.push_str(&format!(" stdout: {stdout}."));
        }
        if !stderr.is_empty() {
            message.push_str(&format!(" stderr: {stderr}."));
        }
        Err(io::Error::new(io::ErrorKind::Other, message))
    }
}

fn delete_reg_key_if_exists(key: &str) -> io::Result<()> {
    let query_output = Command::new("reg").args(["query", key]).output()?;
    if !query_output.status.success() {
        return Ok(());
    }

    run_reg_command(
        &["delete".to_string(), key.to_string(), "/f".to_string()],
        &format!("delete the registry key {key}"),
    )
}
