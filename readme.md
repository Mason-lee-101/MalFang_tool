# MalFang_tool

A Windows and Linux CLI tool for defanging and refanging malware filenames, with optional right-click menus. File contents are unchanged.

## Behavior

```text
Defang: sample.exe        -> sample.exe_<hash>
Refang: sample.exe_<hash> -> <hash>_sample.exe
```

- Supports `md5`, `sha1`, `sha256` (initial default), `sha512`, and `blake3`.
- Verifies existing hashes against file contents. Defang replaces a different supported hash instead of adding another; refang preserves a verified trailing hash, or computes one if absent.
- Uses `_` between the hash and filename, preserves original spaces, and recognizes older space-separated hashes.
- On Windows, defanging changes the final extension. On Linux, defanging clears execute bits and refanging restores them. **Treat refanged files as potentially executable.**
- Renames files in place. Folder operations process direct files only, not subfolders.

## Build

Requires Rust and Cargo:

```bash
cargo build --release
```

Output: `target/release/malfang_tool` (Linux) or `target\release\malfang_tool.exe` (Windows).

## Usage


```text
Usage: malfang_tool [OPTIONS] [PATH]

Arguments:
  [PATH]

Options:
  -a, --hash <ALGORITHM>          Override the saved default for this command (fallback: sha256) [possible values: md5, sha1, sha256, sha512, blake3]
      --default-hash <ALGORITHM>  Save the default algorithm for CLI and default menu actions [possible values: md5, sha1, sha256, sha512, blake3]
      --refang
      --install [<SCOPE>]         Install everything (default), binary + user PATH, or menus + binary without PATH [possible values: all, path, menus]
      --uninstall [<SCOPE>]       Remove everything (default), binary + user PATH, or only menus [possible values: all, path, menus]
  -h, --help                      Print help

Examples:
  Defang: malfang_tool sample.exe
          Renames to sample.exe_<hash>

  Refang: malfang_tool --refang "sample.exe_<hash>"
          Renames to <hash>_sample.exe
```

Linux also provides `--file-manager <FILE_MANAGER>` for installation and removal. Targets include `nautilus`, `nemo`, `caja`, `dolphin`, `thunar`, `xfe`, `pcmanfm`, and `ding`; desktop aliases are `xfdesktop` (Xfce), `plasma` (KDE), and `nemo-desktop` (Cinnamon).


## Install and uninstall

Run as your current user, **without `sudo`** on Linux. Choose one scope; omitting it means `all`.

| Command | Effect |
| --- | --- |
| `--install` | Install binary, user PATH, and context menus |
| `--install path` | Install binary and user PATH only |
| `--install menus` | Install binary and menus without changing PATH |
| `--uninstall` | Remove menus, installed binary, and PATH setup |
| `--uninstall menus` | Remove menus; keep binary and PATH |
| `--uninstall path` | Remove binary and PATH setup; leave menus unchanged |

```bash
malfang_tool --install
malfang_tool --uninstall

# Linux: explicitly target a file manager or desktop
malfang_tool --install menus --file-manager dolphin
malfang_tool --install menus --file-manager xfdesktop
malfang_tool --uninstall menus --file-manager dolphin
```

**Prefer full uninstall if menus are installed:** removing only `path` can leave broken menu entries. Uninstall preserves your samples and unrelated menu entries; it does not undo renamed files. Windows schedules deletion of the running installed binary after exit.

Installed binary locations:

- Windows: `%LOCALAPPDATA%\Programs\MalFang_tool\malfang_tool.exe`
- Linux: `~/.local/bin/malfang_tool`

Menus use the installed copy, so the original can be moved or deleted. Re-run installation from a newer executable to update it; close running installed instances first. Linux PATH setup uses `~/.profile`, `~/.bashrc`, and `~/.zshrc`. After interactive installation with PATH setup, a `[y/N]` prompt offers to start a refreshed shell immediately (default: no). Exiting that shell returns to the original shell with its old PATH; a program cannot update its parent shell. Non-interactive installs never prompt. Otherwise, open a new terminal after PATH changes.

## Context menus

Default **Defang** and **Refang** entries use your saved algorithm (initially SHA-256), with additional algorithm-specific defang entries. Folder selections process direct files only. Menus installed by an older version need reinstalling once to pick up configurable defaults.

### Windows

Explorer menus are registered per user under `HKCU\Software\Classes\*\shell` and `HKCU\Software\Classes\Folder\shell`. On Windows 11, look under **Show more options** or do **Shift+rightclick** to bring up the old menu. Uninstall also cleans up legacy MalFang registrations.

### Linux file managers

The installer detects supported applications automatically; use `--file-manager <TARGET>` to select one explicitly.

| File manager / target | Submenu | Per-user registration |
| --- | --- | --- |
| Nautilus / `nautilus` | Scripts | `$XDG_DATA_HOME/nautilus/scripts` |
| Nemo / `nemo` | Scripts | `$XDG_DATA_HOME/nemo/scripts` |
| Caja / `caja` | Scripts | `$XDG_CONFIG_HOME/caja/scripts` |
| Xfe / `xfe` | Scripts | `~/.config/xfe/scripts` |
| Dolphin (KDE Frameworks 5.85+) / `dolphin` | MalFang | `$XDG_DATA_HOME/kio/servicemenus/malfang-tool.desktop` |
| Thunar / `thunar` | MalFang | `$XDG_CONFIG_HOME/Thunar/uca.xml` |
| PCManFM (GTK/libfm) / `pcmanfm` | MalFang | `$XDG_DATA_HOME/file-manager/actions` |

### Linux desktop icons

Right-click a **file or folder icon**, not the wallpaper. Desktop icons must already be enabled.

| Desktop provider | `--file-manager` target | Menu integration |
| --- | --- | --- |
| Xfce (with Thunar extension support) | `xfdesktop` | MalFang; shared with Thunar |
| KDE Plasma Folder View | `plasma` | MalFang; shared with Dolphin |
| GNOME Desktop Icons NG (DING) | `ding` | Scripts; `~/.local/share/nautilus/scripts` |
| Cinnamon with Nemo | `nemo-desktop` | Scripts; shared with Nemo |
| MATE with Caja | `caja` | Scripts; shared with Caja |

`XDG_DATA_HOME` and `XDG_CONFIG_HOME` default to `~/.local/share` and `~/.config`. Plain GNOME without a desktop icon extension has no desktop file menu. Removing shared menus also affects the associated file manager.

Restart the file manager if menus do not appear. Xfce may require restarting both Thunar and xfdesktop; interactive installation offers separate confirmations. Non-interactive runs do not restart applications. Existing Thunar actions are preserved.

Nautilus, Nemo, and Caja use a newline-delimited selection fallback when no file arguments are supplied; that fallback cannot represent filenames containing newlines.
