#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "malfang-context-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("bin")).unwrap();
        Self(path)
    }

    fn command(&self, program: impl AsRef<Path>) -> Command {
        let mut command = Command::new(program.as_ref());
        command
            .env_clear()
            .env("HOME", &self.0)
            .env("XDG_CONFIG_DIRS", self.0.join("system config"))
            .env("XDG_DATA_DIRS", self.0.join("system data"))
            .env("PATH", self.0.join("bin"))
            .current_dir(&self.0);
        command
    }

    fn cli(&self) -> Command {
        self.command(env!("CARGO_BIN_EXE_MalFang_tool"))
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn optional_binary_install_uses_a_stable_copy() {
    let home = TestHome::new();
    assert_success(
        home.cli()
            .args([
                "--install",
                "menus",
                "--file-manager",
                "nemo",
            ])
            .output()
            .unwrap(),
    );
    let installed = home.0.join(".local/bin/MalFang_tool");
    assert!(installed.is_file());
    assert!(!home.0.join(".profile").exists());
    assert_success(home.command(&installed).args(["--install", "path"]).output().unwrap());
    let profile = fs::read_to_string(home.0.join(".profile")).unwrap();
    assert!(profile.contains(home.0.join(".local/bin").to_str().unwrap()));
    let shell = home.command("/bin/sh")
        .args(["-c", ". \"$HOME/.profile\"; . \"$HOME/.profile\"; printf '%s' \"$PATH\""])
        .output().unwrap();
    assert!(shell.status.success());
    assert_eq!(String::from_utf8(shell.stdout).unwrap(), format!("{}:{}", home.0.join("bin").display(), home.0.join(".local/bin").display()));
    assert_ne!(
        fs::metadata(&installed).unwrap().permissions().mode() & 0o111,
        0
    );
    let script = fs::read_to_string(home.0.join(".local/share/nemo/scripts/Defang")).unwrap();
    assert!(script.contains(installed.to_str().unwrap()));

    // Re-registering from the installed binary must not copy it onto itself.
    assert_success(
        home.command(&installed)
            .args([
                "--install",
                "menus",
                "--file-manager",
                "nemo",
            ])
            .output()
            .unwrap(),
    );
    assert_success(home.command(&installed).args(["--install", "path"]).output().unwrap());
    assert_eq!(
        profile,
        fs::read_to_string(home.0.join(".profile")).unwrap()
    );
    assert_success(home.cli().args(["--uninstall", "menus"]).output().unwrap());
    assert!(installed.is_file());
}

#[test]
fn full_uninstall_removes_installation_and_preserves_user_configuration() {
    let home = TestHome::new();
    let profile = home.0.join(".profile");
    fs::write(&profile, "# User configuration\nexport KEEP_ME=yes\n").unwrap();
    assert_success(home.cli().args(["--install", "path"]).output().unwrap());
    let installed = home.0.join(".local/bin/MalFang_tool");
    let other = home.0.join(".local/bin/other-tool");
    fs::write(&other, "keep").unwrap();
    assert_success(home.command(&installed).arg("--uninstall").output().unwrap());
    assert!(!installed.exists());
    assert!(other.is_file());
    assert_eq!(fs::read_to_string(&profile).unwrap(), "# User configuration\nexport KEEP_ME=yes\n");
    assert_success(home.cli().arg("--uninstall").output().unwrap());
}

#[test]
fn installs_for_managers_with_existing_scripts_or_an_executable() {
    let home = TestHome::new();
    let output = home.cli().args(["--install", "menus"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--file-manager"));

    let data_home = home.0.join("custom data");
    let nemo = data_home.join("nemo/scripts");
    let nautilus = data_home.join("nautilus/scripts");
    let caja = home.0.join(".config/caja/scripts");
    fs::create_dir_all(&nemo).unwrap();
    let launcher = home.0.join("bin/nautilus");
    fs::write(&launcher, "#!/bin/sh\n").unwrap();
    fs::set_permissions(launcher, fs::Permissions::from_mode(0o755)).unwrap();

    assert_success(
        home.cli()
            .env("XDG_DATA_HOME", &data_home)
            .args(["--install", "menus"])
            .output()
            .unwrap(),
    );
    for dir in [&nemo, &nautilus] {
        assert_eq!(fs::read_dir(dir).unwrap().count(), 6);
        let mode = fs::metadata(dir.join("Defang"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    assert!(!caja.exists());

    fs::remove_file(home.0.join("bin/nautilus")).unwrap();
    for _ in 0..2 {
        assert_success(
            home.cli()
                .env("XDG_DATA_HOME", &data_home)
                .args(["--uninstall", "menus"])
                .output()
                .unwrap(),
        );
    }
    assert_eq!(fs::read_dir(nemo).unwrap().count(), 0);
    assert_eq!(fs::read_dir(nautilus).unwrap().count(), 0);
}

#[test]
fn desktop_providers_are_detected_and_can_be_targeted_by_name() {
    let home = TestHome::new();
    for program in ["xfdesktop", "plasmashell", "nemo-desktop"] {
        let launcher = home.0.join("bin").join(program);
        fs::write(&launcher, "#!/bin/sh\n").unwrap();
        fs::set_permissions(launcher, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let ding = home
        .0
        .join("system data/gnome-shell/extensions/ding@rastersoft.com/metadata.json");
    fs::create_dir_all(ding.parent().unwrap()).unwrap();
    fs::write(ding, "{}").unwrap();

    let data = home.0.join("custom data");
    let targets = [
        ("xfdesktop", home.0.join(".config/Thunar/uca.xml")),
        ("plasma", data.join("kio/servicemenus/malfang-tool.desktop")),
        ("nemo-desktop", data.join("nemo/scripts/Defang")),
        ("ding", home.0.join(".local/share/nautilus/scripts/Defang")),
    ];
    assert_success(
        home.cli()
            .env("XDG_DATA_HOME", &data)
            .args(["--install", "menus"])
            .output()
            .unwrap(),
    );
    assert!(!data.join("nautilus/scripts").exists());
    for (target, registration) in targets {
        assert!(fs::read_to_string(&registration)
            .unwrap()
            .contains("Defang"));
        for operation in ["--uninstall", "--install"] {
            assert_success(
                home.cli()
                    .env("XDG_DATA_HOME", &data)
                    .args([operation, "menus", "--file-manager", target])
                    .output()
                    .unwrap(),
            );
            let installed = operation == "--install";
            if target == "xfdesktop" {
                let actions = fs::read_to_string(&registration).unwrap();
                assert_eq!(actions.contains("malfang-tool-defang"), installed);
            } else {
                assert_eq!(registration.is_file(), installed);
            }
        }
    }
}

#[test]
fn wrappers_preserve_literal_paths_and_use_only_their_managers_selection() {
    let home = TestHome::new();
    let executable = home.0.join("MalFang's copy");
    fs::copy(env!("CARGO_BIN_EXE_MalFang_tool"), &executable).unwrap();
    for (manager, scripts_dir, selection_var) in [
        (
            "nautilus",
            ".local/share/nautilus/scripts",
            "NAUTILUS_SCRIPT_SELECTED_FILE_PATHS",
        ),
        (
            "nemo",
            ".local/share/nemo/scripts",
            "NEMO_SCRIPT_SELECTED_FILE_PATHS",
        ),
        (
            "caja",
            "custom config/caja/scripts",
            "CAJA_SCRIPT_SELECTED_FILE_PATHS",
        ),
    ] {
        assert_success(
            home.command(&executable)
                .env("XDG_CONFIG_HOME", home.0.join("custom config"))
                .args(["--install", "menus", "--file-manager", manager])
                .output()
                .unwrap(),
        );
        let wrapper = home.0.join(scripts_dir).join("Defang MD5");
        let literal = format!("{manager} *.bin");
        let decoy = format!("{manager} other.bin");
        let extra = format!("{manager}\tsecond.bin");
        let arg = format!("-{manager}\nargv.bin");
        for file in [&literal, &decoy, &extra, &arg] {
            fs::write(home.0.join(file), b"sample").unwrap();
        }

        let fallback = format!("{literal}\n{extra}\n");
        assert_success(
            home.command(&wrapper)
                .arg(&arg)
                .env(selection_var, &fallback)
                .output()
                .unwrap(),
        );
        assert!(home.0.join(&literal).is_file());
        assert!(!home.0.join(&arg).exists());

        assert_success(
            home.command(&wrapper)
                .env("NAUTILUS_SCRIPT_SELECTED_FILE_PATHS", &decoy)
                .env("NEMO_SCRIPT_SELECTED_FILE_PATHS", &decoy)
                .env("CAJA_SCRIPT_SELECTED_FILE_PATHS", &decoy)
                .env(selection_var, &fallback)
                .output()
                .unwrap(),
        );
        let hash = "5e8ff9bf55ba3508199d22e984129be6";
        for file in [&literal, &extra, &arg] {
            assert!(home.0.join(format!("{file}_{hash}")).is_file());
        }
        assert!(home.0.join(decoy).is_file());
    }
}

#[test]
fn preserves_unrelated_scripts_with_the_same_name() {
    let home = TestHome::new();
    let dir = home.0.join(".config/caja/scripts");
    fs::create_dir_all(&dir).unwrap();
    let script = dir.join("Defang");
    fs::write(&script, "custom action").unwrap();

    let output = home
        .cli()
        .args(["--install", "menus", "--file-manager", "caja"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let output = home
        .cli()
        .args(["--uninstall", "menus", "--file-manager", "caja"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert_success(output);
    assert!(stdout.contains(&format!("Kept {}", script.display())));
    assert!(stdout.contains("removed 0 managed file(s) or action(s)"));
    assert_eq!(fs::read_to_string(script).unwrap(), "custom action");
}

#[test]
fn registers_and_removes_dolphin_pcmanfm_and_xfe_menus() {
    let home = TestHome::new();
    for manager in ["dolphin", "pcmanfm", "xfe"] {
        let launcher = home.0.join("bin").join(manager);
        fs::write(&launcher, "#!/bin/sh\n").unwrap();
        fs::set_permissions(launcher, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let data = home.0.join("custom data");
    for _ in 0..2 {
        assert_success(
            home.cli()
                .env("XDG_DATA_HOME", &data)
                .env("XDG_CONFIG_HOME", home.0.join("custom config"))
                .args(["--install", "menus"])
                .output()
                .unwrap(),
        );
    }

    let dolphin = data.join("kio/servicemenus/malfang-tool.desktop");
    let dolphin_menu = fs::read_to_string(&dolphin).unwrap();
    assert!(dolphin_menu.contains("Type=Service\n"));
    assert!(dolphin_menu.contains("[Desktop Action defang-md5]\nName=Defang MD5\n"));
    assert!(dolphin_menu.contains("/malfang-tool/dolphin/defang-md5\" %F\n"));
    assert_ne!(
        fs::metadata(&dolphin).unwrap().permissions().mode() & 0o111,
        0
    );

    let pcmanfm = data.join("file-manager/actions");
    let pcmanfm_menu = fs::read_to_string(pcmanfm.join("malfang-tool.desktop")).unwrap();
    assert!(pcmanfm_menu.contains("Type=Menu\n"));
    assert!(pcmanfm_menu.contains("malfang-tool-defang-md5;"));
    let pcmanfm_md5 = fs::read_to_string(pcmanfm.join("malfang-tool-defang-md5.desktop")).unwrap();
    assert!(pcmanfm_md5.contains("[X-Action-Profile selection]\n"));
    assert!(pcmanfm_md5.contains("/malfang-tool/pcmanfm/defang-md5' %F\n"));
    fs::write(pcmanfm.join("unrelated.desktop"), "custom action").unwrap();

    let xfe = home.0.join(".config/xfe/scripts/Defang MD5");
    let hash = "5e8ff9bf55ba3508199d22e984129be6";
    for (i, wrapper) in [
        xfe.clone(),
        data.join("malfang-tool/dolphin/defang-md5"),
        data.join("malfang-tool/pcmanfm/defang-md5"),
    ]
    .iter()
    .enumerate()
    {
        let one = format!("--file {i}");
        let two = format!("file {i} [*]");
        fs::write(home.0.join(&one), b"sample").unwrap();
        fs::write(home.0.join(&two), b"sample").unwrap();
        assert_success(home.command(wrapper).args([&one, &two]).output().unwrap());
        assert!(home.0.join(format!("{one}_{hash}")).is_file());
        assert!(home.0.join(format!("{two}_{hash}")).is_file());
    }

    assert_success(
        home.cli()
            .env("XDG_DATA_HOME", &data)
            .args(["--uninstall", "menus"])
            .output()
            .unwrap(),
    );
    assert!(!dolphin.exists());
    assert!(!xfe.exists());
    assert_eq!(fs::read_dir(&pcmanfm).unwrap().count(), 1);
    assert!(pcmanfm.join("unrelated.desktop").is_file());
    for manager in ["dolphin", "pcmanfm"] {
        assert_eq!(
            fs::read_dir(data.join("malfang-tool").join(manager))
                .unwrap()
                .count(),
            0
        );
    }
}

#[test]
fn thunar_install_preserves_custom_actions_and_is_idempotent() {
    let home = TestHome::new();
    let config = home.0.join("custom config");
    let data = home.0.join("data ' quotes & <angle> %F");
    let actions = config.join("Thunar/uca.xml");
    fs::create_dir_all(actions.parent().unwrap()).unwrap();
    let original = "<?xml version=\"1.0\"?>\n<!DOCTYPE actions [<!ELEMENT actions ANY>]>\n\
                    <actions>\n  <!-- keep this -->\n  <action>\n    <name>Existing &amp; useful</name>\n\
                    \x20   <unique-id>existing-1</unique-id>\n  </action>\n</actions>\n";
    fs::write(&actions, original).unwrap();
    let invoke = |option| {
        let output = home
            .cli()
            .env("XDG_DATA_HOME", &data)
            .env("XDG_CONFIG_HOME", &config)
            .args([option, "menus", "--file-manager", "thunar"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert_success(output);
        stdout
    };

    invoke("--install");
    let installed = fs::read_to_string(&actions).unwrap();
    assert!(installed.contains("<!-- keep this -->"));
    assert!(installed.contains("Existing &amp; useful"));
    assert!(installed.contains("&amp; &lt;angle&gt; %%F/malfang-tool/thunar/defang' %F"));
    assert_eq!(installed.matches("<unique-id>malfang-tool-").count(), 6);
    invoke("--install");
    assert_eq!(fs::read_to_string(&actions).unwrap(), installed);

    let stdout = invoke("--uninstall");
    let wrapper = data.join("malfang-tool/thunar/defang");
    assert!(stdout.contains(&format!(
        "Removed 6 MalFang action(s) from {}",
        actions.display()
    )));
    assert!(stdout.contains("kept the configuration file"));
    assert!(stdout.contains(&format!("Removed {}", wrapper.display())));
    assert!(stdout.contains("removed 12 managed file(s) or action(s)"));
    assert_eq!(fs::read_to_string(&actions).unwrap(), original);
    assert!(!wrapper.exists());

    let stdout = invoke("--uninstall");
    assert!(stdout.contains("No MalFang entries removed."));
    assert!(stdout.contains("removed 0 managed file(s) or action(s)"));
}

#[test]
fn thunar_first_install_keeps_the_system_defaults() {
    let home = TestHome::new();
    let defaults = home.0.join("system config/Thunar/uca.xml");
    fs::create_dir_all(defaults.parent().unwrap()).unwrap();
    let original = "<actions><action><name>System default</name></action></actions>";
    fs::write(defaults, original).unwrap();
    for option in ["--install", "--uninstall"] {
        assert_success(
            home.cli()
                .args([option, "menus", "--file-manager", "thunar"])
                .output()
                .unwrap(),
        );
    }
    assert_eq!(
        fs::read_to_string(home.0.join(".config/Thunar/uca.xml")).unwrap(),
        original
    );
}

#[test]
fn refang_adds_a_prefix_from_the_cli_or_context_menu() {
    let home = TestHome::new();
    fs::write(home.0.join("from cli.bin"), b"sample").unwrap();
    fs::write(home.0.join("from menu.bin"), b"sample").unwrap();
    assert_success(
        home.cli()
            .args(["--refang", "--hash", "md5", "from cli.bin"])
            .output()
            .unwrap(),
    );
    assert!(home
        .0
        .join("5e8ff9bf55ba3508199d22e984129be6_from cli.bin")
        .is_file());

    assert_success(
        home.cli()
            .args(["--install", "menus", "--file-manager", "xfe"])
            .output()
            .unwrap(),
    );
    let wrapper = home.0.join(".config/xfe/scripts/Refang");
    assert_success(home.command(wrapper).arg("from menu.bin").output().unwrap());
    let sha256 = "af2bdbe1aa9b6ec1e2ade1d694f41fc71a831d0268e9891562113d8a62add1bf";
    assert!(home.0.join(format!("{sha256}_from menu.bin")).is_file());
}

#[test]
fn defang_clears_all_execute_bits_without_changing_other_permissions() {
    let home = TestHome::new();
    let sample = home.0.join("sample");
    let defanged = home.0.join("sample_5e8ff9bf55ba3508199d22e984129be6");
    fs::write(&sample, b"sample").unwrap();
    for input in [&sample, &defanged] {
        fs::set_permissions(input, fs::Permissions::from_mode(0o751)).unwrap();
        assert_success(
            home.cli()
                .args(["--hash", "md5"])
                .arg(input)
                .output()
                .unwrap(),
        );
        assert!(!sample.exists());
        let mode = fs::metadata(&defanged).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o640);
        assert_eq!(fs::read(&defanged).unwrap(), b"sample");
    }
}
