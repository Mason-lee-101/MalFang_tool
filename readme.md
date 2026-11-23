# MalFang_tool


`MalFang_tool` is a small Rust CLI tool that can be called from a terminal or a context menu. (right click menu) This speeds up malware triage workflow by automating defanging and refanging malware. 

The tool supports both single files and folders:

- If you pass a file, it renames that file.
- If you pass a folder, it renames each file directly inside that folder.
- It does not recurse into subfolders.


## Features

- Supports `md5`, `sha1`, `sha256`, `sha512`, and `blake3`
- Defaults to `sha256`
- Supports `refang` to move a trailing hash to the front of the filename
- Works on Windows and Linux
- Adds context-menu entries on Windows and common Linux file managers
- Leaves file contents unchanged

## Defanging behavior

When Defanging malware it will appends the hash of the file to the filename.

Examples:

```
sample.exe` -> `sample.exe <hash>
```

On `Windows`, Since the hash is appends to the very end of the file name, It will stop windows from treating the file as an executable.

On Linux, defanging clears the owner, group, and other execute bits (equivalent to `chmod a-x`)

When defanging, the tool checks the file's `md5`, `sha1`, `sha256`, `sha512`, and `blake3` values. If one of those hashes is already at the end of the filename and it matches the selected hash, the tool leaves the filename unchanged. If the trailing hash uses a different supported algorithm, defang replaces it with the newly selected hash. If a supported hash is at the front, defang normalizes the name back to the trailing form and uses the selected hash for the final suffix.

`Refang` moves a supported trailing hash from the end of the filename to the front:

- `sample.exe <hash>` -> `<hash> sample.exe`

On `Linux`, refanging also restores execute bits on the renamed file (`chmod +x`).



## Build

Requirements:

- Rust toolchain
- Cargo

Build a release binary:

```bash
cargo build --release
```

Binary location:

- Windows: `target\release\MalFang_tool.exe`
- Linux: `target/release/MalFang_tool`

### Repository layout

- `src/main.rs`: command-line arguments and command dispatch
- `src/rename.rs`: shared hashing and filename rules
- `src/platform/linux/`: file-manager registrations, script template, and desktop refresh
- `src/platform/windows.rs`: Explorer registry integration
- `tests/`: integration tests

## Usage

```bash
MalFang_tool [OPTIONS] [PATH]
```

Options:

- `-a, --hash <ALGORITHM>`: `md5`, `sha1`, `sha256`, `sha512`, or `blake3`
- `--refang`: move a trailing supported hash to the front as `<hash> <filename>`
- `--install-context-menu`: install context-menu entries for the current platform
- `--install-binary` (with `--install-context-menu`): copy the executable to a stable per-user location and register menus using that copy
- `--uninstall-context-menu`: remove context-menu entries for the current platform
- `--file-manager <TARGET>` (Linux only, with install or uninstall): target `nautilus`, `nemo`, `caja`, `dolphin`, `thunar`, `xfe`, `pcmanfm`, or a desktop target listed below

Default algorithm:

```bash
sha256
```

## Examples

Rename one file with the default hash:

```bash
MalFang_tool suspicious.bin
```

Rename one file with BLAKE3:

```bash
MalFang_tool --hash blake3 suspicious.bin
```

Move a trailing hash to the front:

```bash
MalFang_tool --refang "suspicious.bin <hash>"
```

Rename all files directly inside a folder:

```bash
MalFang_tool samples
```


## Context menu (right click menu)

### Windows

On Windows, the tool installs Explorer context-menu entries under `HKEY_CURRENT_USER` (`HKCU`), so the changes apply only to the current user. Installation reports the registry paths where file and folder context menus were installed, without listing menu options or individual registry operations. Keep the executable in that location, or re-run installation after moving it. On Windows 11, the entries may appear under **Show more options**.

The installer writes the `MalFangToolDefang`, `MalFangToolDefangOther`, and `MalFangToolRefang` subkeys under both of these locations:

- Files: `HKEY_CURRENT_USER\Software\Classes\*\shell` (the `*` is a literal registry key)
- Folders: `HKEY_CURRENT_USER\Software\Classes\Folder\shell`

Uninstall removes those subkeys and the legacy `HashRenameDefang`, `HashRenameDefangOther`, and `HashRenameUnfang` subkeys from both locations. It also checks `HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\*\shell` for the same keys. The parent `shell` keys are left in place.

Install:

```powershell
MalFang_tool.exe --install-context-menu
```

Optionally copy the executable to `%LOCALAPPDATA%\Programs\MalFang_tool\MalFang_tool.exe` and point the menus there:

```powershell
MalFang_tool.exe --install-context-menu --install-binary
```

You can then move or delete the original executable without breaking the menus. This does not modify `PATH`. Re-run from a newer executable to update the copy; close running installed instances first. Context-menu uninstall leaves the installed executable in place.

Remove:

```powershell
MalFang_tool.exe --uninstall-context-menu
```

Installed menu behavior:

- `Defang file` / `Defang folder` uses `sha256`
- `Refang file` / `Refang folder` moves a trailing hash to the front
- `Defang ... with other hashes` adds submenu entries for `md5`, `sha1`, `sha512`, and `blake3`

### Linux

Linux context menus depend on your file manager and desktop environment. MalFang detects supported applications and installs the appropriate menu entries for your user.

#### Install

Run the installer as your desktop user, **without `sudo`**. Menu entries point to the executable used during installation, so keep it in a permanent location.

```bash
MalFang_tool --install-context-menu
```

Optionally copy the executable to `~/.local/bin/MalFang_tool` and point the menus there:

```bash
MalFang_tool --install-context-menu --install-binary
```

You can then move or delete the original executable without breaking the menus. The installed copy must stay in place. This does not modify `PATH`; menus use the installed copy's absolute path. Re-run from a newer executable to update the copy. Context-menu uninstall leaves the installed executable in place.

The installer detects supported file managers and desktops from executables on `PATH`, existing menu configuration, or an installed Desktop Icons NG extension. To install for a specific target or bypass detection:

```bash
MalFang_tool --install-context-menu --file-manager dolphin
```

#### Supported file managers

| File manager | Right-click submenu | Per-user registration |
| --- | --- | --- |
| Nautilus | Scripts | `$XDG_DATA_HOME/nautilus/scripts` |
| Nemo | Scripts | `$XDG_DATA_HOME/nemo/scripts` |
| Caja | Scripts | `$XDG_CONFIG_HOME/caja/scripts` |
| Xfe | Scripts | `~/.config/xfe/scripts` |
| Dolphin (KDE Frameworks 5.85+) | MalFang | `$XDG_DATA_HOME/kio/servicemenus/malfang-tool.desktop` |
| Thunar | MalFang | `$XDG_CONFIG_HOME/Thunar/uca.xml` |
| PCManFM (GTK/libfm) | MalFang | `$XDG_DATA_HOME/file-manager/actions` |

`XDG_DATA_HOME` and `XDG_CONFIG_HOME` default to `~/.local/share` and `~/.config`. Restart the file manager if the actions do not appear. For Thunar, run `thunar --quit` and reopen it to reload custom actions. Existing Thunar actions are preserved.


#### Using the menus

Right-click a local file or folder and open the submenu listed above:

- **Defang** uses SHA-256 by default; other entries offer MD5, SHA-1, SHA-512, and BLAKE3.
- **Refang** moves a verified hash to the front of the filename.
- Folder selections process files directly inside the folder, not nested folders.

Nautilus, Nemo, and Caja fall back to a newline-delimited selection when no file arguments are provided. This fallback cannot represent filenames containing newlines.

#### Uninstall

Remove MalFang's actions from all supported managers:

```bash
MalFang_tool --uninstall-context-menu
```

To remove entries for only one target:

```bash
MalFang_tool --uninstall-context-menu --file-manager dolphin
```

This removes menu registrations and managed scripts, not the executable or your samples. Unrelated entries and existing Thunar actions are preserved.

#### Desktop icons

Right-click a **file or folder icon** on the desktop rather than looking in the wallpaper menu. The installer does not enable desktop icons, install extensions, or restart your desktop session.

| Desktop icon provider | `--file-manager` target | Where to look |
| --- | --- | --- |
| Xfce (`xfdesktop` with Thunar extension support) | `xfdesktop` | **MalFang**; uses the same custom actions as Thunar |
| KDE Plasma with Folder View | `plasma` | **MalFang**; uses the same service menu as Dolphin |
| GNOME with Desktop Icons NG (DING) enabled | `ding` | **Scripts**; uses `~/.local/share/nautilus/scripts` |
| Cinnamon with Nemo managing desktop icons | `nemo-desktop` | **Scripts**; uses the same scripts as Nemo |
| MATE with Caja managing desktop icons | `caja` | **Scripts**; uses the same scripts as Caja |

On Xfce, restarting **Thunar** does not reload an already-running **xfdesktop**. When installing or removing their menu entries from an interactive graphical terminal, MalFang offers two separate confirmations if it finds the applications running: quit Thunar (reopen it yourself) and restart xfdesktop. Press Enter to accept or answer `n` to skip either one. Non-interactive runs never close or restart applications.

If you skip the restart, or if the installer cannot detect the running applications, you can sign out and back in or restart just the desktop icon process from a terminal:

```bash
xfdesktop --quit && xfdesktop
```

Plain GNOME without a desktop icon extension has no desktop file menu. Removing a shared entry also removes it from the associated file manager (for example, DING and Nautilus share a directory when `XDG_DATA_HOME` is unset).

## Notes

- Folder mode only processes files in the selected directory, not nested folders.
- The tool renames files in place.
- This project currently supports only Windows and Linux.
