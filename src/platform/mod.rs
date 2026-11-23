#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub(crate) use linux::{install_context_menu, rename_file, uninstall_context_menu, FileManager};
#[cfg(target_os = "windows")]
pub(crate) use windows::{install_context_menu, rename_file, uninstall_context_menu};
