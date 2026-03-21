# Defang File And Folder Tool

`Defang file and folder tool` is a small Rust CLI that defangs files by renaming them to include the hash of the file. I got lazy about renaming files and kept track of which file I was working on while reverse-engineering malware. This tool is 90% AI made. I just need the tool to work. 

The tool supports both single files and folders:

- If you pass a file, it renames that file.
- If you pass a folder, it renames each file directly inside that folder.
- It does not recurse into subfolders.

## Features

- Supports `md5`, `sha1`, `sha256`, `sha512`, and `blake3`
- Defaults to `sha256`
- Works on Windows and Linux
- Adds context-menu entries on Windows and common Linux file managers
- Leaves file contents unchanged

## Rename behavior

The hash is appended to the existing filename.

Examples:

- Windows: `sample.exe` -> `sample.exe <hash>`
- Linux: `sample` -> `sample <hash>`

On Linux, defanging also removes execute bits from the renamed file (`chmod a-x`) so binaries and scripts stop being directly executable.

If a filename already ends with the same hash suffix for the selected platform separator, the tool leaves it unchanged.

## Build

Requirements:

- Rust toolchain
- Cargo

Build a release binary:

```bash
cargo build --release
```

Binary location:

- Windows: `target\release\hashrename.exe`
- Linux: `target/release/hashrename`

## Usage

```bash
hashrename [OPTIONS] [PATH]
```

Options:

- `-a, --hash <ALGORITHM>`: `md5`, `sha1`, `sha256`, `sha512`, or `blake3`
- `--install-context-menu`: install context-menu entries for the current platform
- `--uninstall-context-menu`: remove context-menu entries for the current platform

Default algorithm:

```bash
sha256
```

## Examples

Rename one file with the default hash:

```bash
hashrename suspicious.bin
```

Rename one file with BLAKE3:

```bash
hashrename --hash blake3 suspicious.bin
```

Rename all files directly inside a folder:

```bash
hashrename samples
```

Run with Cargo during development:

```bash
cargo run -- suspicious.bin
```

## Context menu

### Windows

On Windows, the tool adds Explorer entries for files and folders under the current user registry hive.

Install:

```powershell
hashrename.exe --install-context-menu
```

Remove:

```powershell
hashrename.exe --uninstall-context-menu
```

Installed menu behavior:

- `Defang file` / `Defang folder` uses `sha256`
- `Defang ... with other hashes` adds submenu entries for `md5`, `sha1`, `sha512`, and `blake3`

### Linux

On Linux, the tool installs per-user file-manager scripts for Nautilus, Nemo, and Caja:

- `~/.local/share/nautilus/scripts`
- `~/.local/share/nemo/scripts`
- `~/.config/caja/scripts`

Install:

```bash
hashrename --install-context-menu
```

Remove:

```bash
hashrename --uninstall-context-menu
```

Installed menu behavior:

- Adds `Defang` for the default `sha256` flow
- Adds separate entries for `md5`, `sha1`, `sha512`, and `blake3`
- The entries run from the file manager `Scripts` submenu

## Notes

- Folder mode only processes files in the selected directory, not nested folders.
- The tool renames files in place.
- This project currently supports only Windows and Linux.
