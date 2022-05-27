use winapi::shared::windef::HWND;
use winapi::um::winnt::LPCSTR;

use super::SquadEx;

#[inline]
unsafe fn dpi(window: HWND) -> u32 {
	// 120 for me at 2560x1440
	winapi::um::winuser::GetDpiForWindow(window)
}

// FIXME fullscreen doesn't work
// FIXME DPI at 1080p

#[inline]
unsafe fn window_size(window: HWND) -> Option<(u32, u32, u32, u32)> {
	let mut rect: winapi::shared::windef::RECT = std::mem::zeroed();
	if winapi::um::winuser::GetClientRect(window, &mut rect) != winapi::shared::minwindef::TRUE {
		return None;
	}

	let mut point: winapi::shared::windef::POINT = std::mem::zeroed();
	if winapi::um::winuser::ClientToScreen(window, &mut point) != winapi::shared::minwindef::TRUE {
		return None;
	}

	Some((
		point.x.try_into().ok()?,
		point.y.try_into().ok()?,
		rect.right as _,
		rect.bottom as _
	))
}

pub fn get() -> Option<SquadEx> {
	unsafe {
		let window: HWND = winapi::um::winuser::FindWindowA(
			"UnrealWindow\0".as_ptr() as LPCSTR,
			"SquadGame  \0".as_ptr() as LPCSTR
		);

		if window.is_null() {
			return None;
		}

		Some(SquadEx {
			dpi: dpi(window),
			window: window_size(window),
		})
	}
}