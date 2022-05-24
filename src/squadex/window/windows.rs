use winapi::shared::windef::HWND;
use winapi::um::winnt::LPCSTR;

use super::SquadEx;

// FIXME In Windows Vista and later, the Window Rect now includes the area occupied by the drop shadow.
// https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect

#[inline]
unsafe fn dpi(window: HWND) -> u32 {
	// 120 for me at 2560x1440
	winapi::um::winuser::GetDpiForWindow(window)
}

#[inline]
unsafe fn window_size(window: HWND) -> Option<(u32, u32, u32, u32)> {
	let mut rect: winapi::shared::windef::RECT = std::mem::zeroed();
	if winapi::um::winuser::GetWindowRect(window, &mut rect) != winapi::shared::minwindef::TRUE {
		return None;
	}

	Some((
		rect.left.try_into().ok()?,
		rect.top.try_into().ok()?,
		rect.right.checked_sub(rect.left)?.try_into().ok()?,
		rect.bottom.checked_sub(rect.top)?.try_into().ok()?
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