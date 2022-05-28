#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::frame;

#[cfg(not(target_os = "windows"))]
mod fallback;
#[cfg(not(target_os = "windows"))]
pub use fallback::frame;