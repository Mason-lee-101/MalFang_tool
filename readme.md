# MalFang_tool


`MalFang_tool` is a small Rust CLI tool that can be called from a terminal or a context menu. (right click menu) This speeds up malware triage workflow by automating defanging and refanging malware. 

The tool supports both single files and folders:

- If you pass a file, it renames that file.
- If you pass a folder, it renames each file directly inside that folder.
- It does not recurse into subfolders.

### Why?

I got lazy when it came to renaming files and keeping track of malware samples, which had the exact same file names while reverse-engineering malware. So this tool is 90% AI made, as I need the tool to work for this group of samples.



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

- Windows: `sample.exe` -> `sample.exe <hash>`
- Linux: `sample` -> `sample <hash>`

On `Windows`, Since the hash is appends to the very end of the file name, It will stop windows from treating the file as an executable.

On `Linux`, defanging also removes execute bits from the renamed file (`chmod a-x`) so binaries and scripts stop being directly executable.

When defanging, the tool checks the file's `md5`, `sha1`, `sha256`, `sha512`, and `blake3` values. If one of those hashes is already at the end of the filename and it matches the selected hash, the tool leaves the file unchanged. If the trailing hash uses a different supported algorithm, defang replaces it with the newly selected hash. If a supported hash is at the front, defang normalizes the name back to the trailing form and uses the selected hash for the final suffix.

Refang moves a supported trailing hash from the end of the filename to the front:

- `sample.exe <hash>` -> `<hash> sample.exe`

On `Linux`, refanging also restores execute bits on the renamed file (`chmod +x`).

If the hash is already at the front, refang leaves the file unchanged.

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

## Usage

```bash
MalFang_tool [OPTIONS] [PATH]
```

Options:

- `-a, --hash <ALGORITHM>`: `md5`, `sha1`, `sha256`, `sha512`, or `blake3`
- `--refang`: move a trailing supported hash to the front as `<hash> <filename>`
- `--install-context-menu`: install context-menu entries for the current platform
- `--uninstall-context-menu`: remove context-menu entries for the current platform

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

Run with Cargo during development:

```bash
cargo run -- suspicious.bin
```

## Context menu

### Windows

On Windows, the tool adds Explorer entries for files and folders under the current user registry hive.

Install:

```powershell
MalFang_tool.exe --install-context-menu
```

Remove:

```powershell
MalFang_tool.exe --uninstall-context-menu
```

Installed menu behavior:

- `Defang file` / `Defang folder` uses `sha256`
- `Refang file` / `Refang folder` moves a trailing hash to the front
- `Defang ... with other hashes` adds submenu entries for `md5`, `sha1`, `sha512`, and `blake3`

### Linux

On Linux, the tool installs per-user file-manager scripts for Nautilus, Nemo, and Caja:

- `~/.local/share/nautilus/scripts`
- `~/.local/share/nemo/scripts`
- `~/.config/caja/scripts`

Install:

```bash
MalFang_tool --install-context-menu
```

Remove:

```bash
MalFang_tool --uninstall-context-menu
```

Installed menu behavior:

- Adds `Defang` for the default `sha256` flow
- Adds `Refang` to move a trailing hash to the front
- Adds separate entries for `md5`, `sha1`, `sha512`, and `blake3`
- The entries run from the file manager `Scripts` submenu

## Notes

- Folder mode only processes files in the selected directory, not nested folders.
- The tool renames files in place.
- This project currently supports only Windows and Linux.
