#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::get;

#[cfg(not(target_os = "windows"))]
mod fallback;
#[cfg(not(target_os = "windows"))]
pub use fallback::get;

#[derive(Debug)]
pub struct SquadEx {
	pub dpi: u32,
	pub window: Option<(u32, u32, u32, u32)>
}