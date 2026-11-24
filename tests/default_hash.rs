use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_MalFang_tool"));
    command
        .env("HOME", home)
        .env("APPDATA", home)
        .env("XDG_CONFIG_HOME", home.join("config"));
    command
}

fn config_path(home: &Path) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        home.join("MalFang_tool/config.toml")
    }
    #[cfg(target_os = "linux")]
    {
        home.join("config/malfang-tool/config.toml")
    }
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const MD5: &str = "5e8ff9bf55ba3508199d22e984129be6";
const SHA256: &str = "af2bdbe1aa9b6ec1e2ade1d694f41fc71a831d0268e9891562113d8a62add1bf";

#[test]
fn saved_default_applies_to_defang_refang_and_can_be_overridden() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    let input = home.join("fallback.bin");
    fs::write(&input, b"sample").unwrap();
    success(cli(home).arg(&input).output().unwrap());
    assert!(home.join(format!("fallback.bin_{SHA256}")).is_file());

    success(cli(home).args(["--default-hash", "md5"]).output().unwrap());
    assert!(fs::read_to_string(config_path(home))
        .unwrap()
        .contains("default_hash = \"md5\""));
    let input = home.join("saved.bin");
    fs::write(&input, b"sample").unwrap();
    success(cli(home).arg(&input).output().unwrap());
    assert!(home.join(format!("saved.bin_{MD5}")).is_file());

    let input = home.join("override.bin");
    fs::write(&input, b"sample").unwrap();
    success(
        cli(home)
            .args(["--hash", "sha256"])
            .arg(input)
            .output()
            .unwrap(),
    );
    assert!(home.join(format!("override.bin_{SHA256}")).is_file());

    let input = home.join("refang.bin");
    fs::write(&input, b"sample").unwrap();
    success(cli(home).arg("--refang").arg(input).output().unwrap());
    assert!(home.join(format!("{MD5}_refang.bin")).is_file());

    success(
        cli(home)
            .args(["--default-hash", "sha256"])
            .output()
            .unwrap(),
    );
    // Verified trailing hashes are preserved by refang, regardless of the saved default.
    success(
        cli(home)
            .arg("--refang")
            .arg(home.join(format!("saved.bin_{MD5}")))
            .output()
            .unwrap(),
    );
    assert!(home.join(format!("{MD5}_saved.bin")).is_file());
    let input = home.join("reset.bin");
    fs::write(&input, b"sample").unwrap();
    success(cli(home).arg(input).output().unwrap());
    assert!(home.join(format!("reset.bin_{SHA256}")).is_file());
}

#[test]
fn invalid_config_reports_an_error_but_explicit_hash_can_bypass_it() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    let config = config_path(home);
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "default_hash = 'unknown'\n").unwrap();
    let input = home.join("sample.bin");
    fs::write(&input, b"sample").unwrap();
    let output = cli(home).arg(&input).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("default_hash"));
    assert!(input.is_file());
    success(
        cli(home)
            .args(["--hash", "md5"])
            .arg(input)
            .output()
            .unwrap(),
    );
    // Saving a valid default repairs an invalid algorithm value.
    success(cli(home).args(["--default-hash", "md5"]).output().unwrap());
    fs::write(&config, "invalid toml = [").unwrap();
    assert!(!cli(home)
        .args(["--default-hash", "sha256"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(fs::read_to_string(config).unwrap(), "invalid toml = [");
}

#[cfg(target_os = "linux")]
#[test]
fn menus_read_changed_default_without_reinstallation() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    let data = home.join("data");
    let mut install = cli(home);
    install.env("XDG_DATA_HOME", &data);
    success(
        install
            .args(["--install", "menus", "--file-manager", "nemo"])
            .output()
            .unwrap(),
    );
    success(cli(home).args(["--default-hash", "md5"]).output().unwrap());
    for (menu, name, expected) in [
        ("Defang", "default.bin", format!("default.bin_{MD5}")),
        ("Refang", "refang.bin", format!("{MD5}_refang.bin")),
        (
            "Defang SHA-1",
            "explicit.bin",
            "explicit.bin_8151325dcdbae9e0ff95f9f9658432dbedfdb209".to_string(),
        ),
    ] {
        let input = home.join(name);
        fs::write(&input, b"sample").unwrap();
        let mut command = Command::new(data.join("nemo/scripts").join(menu));
        command
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"));
        success(command.arg(input).output().unwrap());
        assert!(home.join(expected).is_file());
    }
}
