use super::{
    atomic_write, install_managed_file, remove_managed_file, shell_single_quote, FileManager,
    MANAGED_MARKER, SCRIPT_ENTRIES,
};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const PCMANFM_MENU: &str = "malfang-tool.desktop";
const THUNAR_ID_PREFIX: &str = "malfang-tool-";

pub(super) fn install(manager: FileManager, path: &Path, wrappers: &Path) -> io::Result<()> {
    match manager {
        FileManager::Dolphin => install_dolphin(path, wrappers),
        FileManager::Thunar => update_thunar(path, Some(wrappers)).map(|_| ()),
        FileManager::Pcmanfm => install_pcmanfm(path, wrappers),
        _ => unreachable!("script managers do not have native action files"),
    }
}

pub(super) fn uninstall(manager: FileManager, path: &Path) -> io::Result<usize> {
    match manager {
        FileManager::Dolphin => remove_managed_file(path),
        FileManager::Pcmanfm => {
            let mut removed = remove_managed_file(&path.join(PCMANFM_MENU))?;
            for entry in SCRIPT_ENTRIES {
                removed += remove_managed_file(&path.join(format!("malfang-tool-{}.desktop", entry.id)))?;
            }
            Ok(removed)
        }
        _ => unreachable!("only Dolphin and PCManFM use standalone action files"),
    }
}

fn install_dolphin(path: &Path, wrappers: &Path) -> io::Result<()> {
    let mut menu = format!(
        "{MANAGED_MARKER}[Desktop Entry]\nType=Service\nName=MalFang\n\
         MimeType=application/octet-stream;inode/directory;\n\
         X-KDE-Protocol=file\nX-KDE-Submenu=MalFang\nActions="
    );
    for entry in SCRIPT_ENTRIES {
        write!(menu, "{};", entry.id).unwrap();
    }
    menu.push('\n');
    for entry in SCRIPT_ENTRIES {
        let exec = desktop_value(&dolphin_exec(&wrappers.join(entry.id))?);
        write!(
            menu,
            "\n[Desktop Action {}]\nName={}\nExec={exec}\n",
            entry.id, entry.file_name,
        )
        .unwrap();
    }
    // User-installed Dolphin service menus must be executable to be trusted.
    install_managed_file(path, &menu, 0o755)
}

fn install_pcmanfm(path: &Path, wrappers: &Path) -> io::Result<()> {
    let mut menu = format!(
        "{MANAGED_MARKER}[Desktop Entry]\nType=Menu\nName=MalFang\n\
         Schemes=file;\nSelectionCount=>0\nItemsList="
    );
    for entry in SCRIPT_ENTRIES {
        let exec = desktop_value(&glib_exec(&wrappers.join(entry.id))?);
        let action = format!(
            "{MANAGED_MARKER}[Desktop Entry]\nType=Action\nName={}\n\
             TargetContext=true\nProfiles=selection;\n\n\
             [X-Action-Profile selection]\nExec={exec}\n\
             Schemes=file;\nSelectionCount=>0\n",
            entry.file_name,
        );
        let id = format!("malfang-tool-{}", entry.id);
        install_managed_file(&path.join(format!("{id}.desktop")), &action, 0o644)?;
        write!(menu, "{id};").unwrap();
    }
    menu.push('\n');
    install_managed_file(&path.join(PCMANFM_MENU), &menu, 0o644)
}

fn exec_path(path: &Path) -> io::Result<String> {
    let path = path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "menu wrapper path is not UTF-8")
    })?;
    // Prevent literal percents in the path from becoming file-manager field codes.
    Ok(path.replace('%', "%%"))
}

fn glib_exec(wrapper: &Path) -> io::Result<String> {
    Ok(format!("{} %F", shell_single_quote(&exec_path(wrapper)?)))
}

fn dolphin_exec(wrapper: &Path) -> io::Result<String> {
    // The desktop-entry Exec grammar uses double quotes, unlike GLib's shell parser.
    let mut quoted = String::from("\"");
    for character in exec_path(wrapper)?.chars() {
        if matches!(character, '"' | '\\' | '$' | '`') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push_str("\" %F");
    Ok(quoted)
}

fn desktop_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub(super) fn update_thunar(path: &Path, wrappers: Option<&Path>) -> io::Result<usize> {
    let (original, mode) = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => (
            fs::read_to_string(path)?,
            metadata.permissions().mode() & 0o777,
        ),
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Thunar config is not a regular file: {}", path.display()),
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if wrappers.is_none() {
                return Ok(0);
            }
            (thunar_defaults()?, 0o600)
        }
        Err(e) => return Err(e),
    };

    let (updated, removed) = thunar_xml(&original, wrappers).map_err(|err| {
        io::Error::new(err.kind(), format!("{}: {err}", path.display()))
    })?;
    if updated != original {
        atomic_write(path, &updated, mode)?;
    }
    Ok(removed)
}

fn thunar_defaults() -> io::Result<String> {
    let config_dirs = env::var_os("XDG_CONFIG_DIRS")
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/etc/xdg".into());
    for dir in env::split_paths(&config_dirs).filter(|dir| dir.is_absolute()) {
        match fs::read_to_string(dir.join("Thunar/uca.xml")) {
            Ok(defaults) => return Ok(defaults),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n</actions>\n".to_string())
}

fn thunar_xml(original: &str, wrappers: Option<&Path>) -> io::Result<(String, usize)> {
    // Thunar's own defaults contain an internal DTD. No external resolver is used.
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(original, options)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let root = doc.root_element();
    if !root.has_tag_name("actions") || root.tag_name().namespace().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "expected <actions>"));
    }

    let mut updated = original.to_string();
    let mut removed = 0;
    // Edit only our action ranges, retaining the rest of the user's XML verbatim.
    for action in root.children().filter(|node| node.has_tag_name("action")).rev() {
        let owned = action.children().any(|node| {
            node.has_tag_name("unique-id")
                && node
                    .text()
                    .and_then(|id| id.trim().strip_prefix(THUNAR_ID_PREFIX))
                    .is_some_and(|id| SCRIPT_ENTRIES.iter().any(|entry| entry.id == id))
        });
        if owned {
            let mut range = action.range();
            if original[..range.start].ends_with('\n') {
                range.start -= 1;
            }
            updated.replace_range(range, "");
            removed += 1;
        }
    }

    if let Some(wrappers) = wrappers {
        // The removed actions were inside the root, so only its end offset shifts.
        let root_end = root.range().end - (original.len() - updated.len());
        let root_xml = &updated[root.range().start..root_end];
        let additions = thunar_entries(wrappers)?;
        if root_xml.ends_with("/>") {
            updated.replace_range(root_end - 2..root_end, &format!(">{additions}\n</actions>"));
        } else if let Some(close) = root_xml.rfind("</actions") {
            // Leave any whitespace before the closing tag in place on uninstall.
            let close = root.range().start + close;
            let insert = updated[..close].trim_end_matches(char::is_whitespace).len();
            updated.insert_str(insert, &additions);
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "missing </actions>"));
        }
    }
    Ok((updated, removed))
}

fn thunar_entries(wrappers: &Path) -> io::Result<String> {
    let mut entries = String::new();
    for entry in SCRIPT_ENTRIES {
        let command = glib_exec(&wrappers.join(entry.id))?
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        write!(
            entries,
            "
<action>
  <name>{}</name>
  <submenu>MalFang</submenu>
  <unique-id>{THUNAR_ID_PREFIX}{}</unique-id>
  <command>{command}</command>
  <patterns>*</patterns>
  <directories/><audio-files/><image-files/>
  <other-files/><text-files/><video-files/>
</action>",
            entry.file_name, entry.id,
        )
        .unwrap();
    }
    Ok(entries)
}
