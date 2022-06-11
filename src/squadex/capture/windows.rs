use crate::prelude::*;

use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::windef::{HDC, HMONITOR, HWND, RECT};
use winapi::um::winnt::LPCSTR;

type Frame = image::ImageBuffer<image::Bgra<u8>, Box<[u8]>>;

static BLACKOUT: SusRefCell<Blackout> = SusRefCell::new(Blackout::None);

#[derive(Clone, Copy, Debug)]
enum Blackout {
	None,
	Ok(HWND),
	Blackout(HWND)
}
impl Blackout {
	#[inline]
	fn is_blackout(window: HWND) -> bool {
		match *BLACKOUT.borrow() {
			Blackout::None | Blackout::Ok(_) => false,
			Blackout::Blackout(handle) => handle == window
		}
	}

	#[inline]
	#[must_use]
	fn update(image: &Frame, window: Option<HWND>) -> bool {
		let mut blackout = BLACKOUT.borrow_mut();
		if let Some(window) = window {
			let needs_update = match *blackout {
				Blackout::None => true,
				Blackout::Ok(handle) | Blackout::Blackout(handle) => handle != window
			};
			if needs_update {
				if image.par_iter().step_by(4).all(|byte| *byte == 0) {
					*blackout = Blackout::Blackout(window);
					return true;
				} else {
					*blackout = Blackout::Ok(window);
				}
			}
		} else {
			*blackout = Blackout::None;
		}
		false
	}
}

macro_rules! os_err {
	($other:literal) => {{
		let mut err = std::io::Error::last_os_error();
		if let Some(errno) = err.raw_os_error() {
			if errno == 0 {
				err = std::io::Error::new(std::io::ErrorKind::Other, $other);
			}
		}
		err
	}};
}

fn find_primary_display(hdc: HDC) -> Option<BBox<i32>> {
	unsafe {
		let mut rect: Option<RECT> = None;

		unsafe extern "system" fn enumerate(monitor: HMONITOR, _hdc: HDC, rect: *mut RECT, primary_monitor: isize) -> i32 {
			let primary_monitor = primary_monitor as usize as *mut Option<RECT>;

			let mut info: winapi::um::winuser::MONITORINFO = core::mem::zeroed();
			info.cbSize = core::mem::size_of::<winapi::um::winuser::MONITORINFO>() as _;

			if winapi::um::winuser::GetMonitorInfoA(monitor, &mut info) == 0 {
				return TRUE;
			}

			if info.dwFlags & winapi::um::winuser::MONITORINFOF_PRIMARY != 0 {
				*primary_monitor = Some(*rect);
				return FALSE;
			}

			TRUE
		}

		winapi::um::winuser::EnumDisplayMonitors(hdc, core::ptr::null_mut(), Some(enumerate), &mut rect as *mut Option<RECT> as usize as _);

		rect.map(|rect| BBox {
			x: rect.left,
			y: rect.top,
			w: rect.right - rect.left,
			h: rect.bottom - rect.top,
		})
	}
}

fn find_window() -> Option<HWND> {
	unsafe {
		let window: HWND = winapi::um::winuser::FindWindowA("UnrealWindow\0".as_ptr() as LPCSTR, "SquadGame  \0".as_ptr() as LPCSTR);

		if window.is_null() {
			None
		} else {
			Some(window)
		}
	}
}

fn window_bounds(window: HWND) -> Result<BBox<i32>, std::io::Error> {
	unsafe {
		let mut rect: winapi::shared::windef::RECT = std::mem::zeroed();
		if winapi::um::winuser::GetClientRect(window, &mut rect) != winapi::shared::minwindef::TRUE {
			return Err(os_err!("GetClientRect failed"));
		}

		let mut point: winapi::shared::windef::POINT = std::mem::zeroed();
		if winapi::um::winuser::ClientToScreen(window, &mut point) != winapi::shared::minwindef::TRUE {
			return Err(os_err!("ClientToScreen failed"));
		}

		Ok(BBox {
			x: point.x,
			y: point.y,
			w: rect.right,
			h: rect.bottom,
		})
	}
}

#[inline]
fn screen_bounds() -> BBox<i32> {
	unsafe {
		BBox {
			x: winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_XVIRTUALSCREEN),
			y: winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_YVIRTUALSCREEN),
			w: winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_CXVIRTUALSCREEN),
			h: winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_CYVIRTUALSCREEN),
		}
	}
}

pub fn frame() -> Result<Frame, anyhow::Error> {
	unsafe {
		#[cfg(test)]
		winapi::um::winuser::SetProcessDpiAwarenessContext(winapi::shared::windef::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

		let raw_window = find_window();
		let mut window_handle = raw_window;

		let (hdc_screen, bounds, clip) = match window_handle {
			Some(window) => {
				let bounds = window_bounds(window)?;
				if Blackout::is_blackout(window) {
					window_handle = None;
					let hdc_screen = winapi::um::winuser::GetDC(core::ptr::null_mut());
					(hdc_screen, screen_bounds(), Some(bounds))
				} else {
					(winapi::um::winuser::GetDC(window), bounds, None)
				}
			}

			None => {
				let hdc_screen = winapi::um::winuser::GetDC(core::ptr::null_mut());
				(hdc_screen, screen_bounds(), find_primary_display(hdc_screen))
			}
		};

		let hdc_screen = match UniquePtr::new_nullable(hdc_screen, |hdc_screen| {
			winapi::um::winuser::ReleaseDC(window_handle.unwrap_or(core::ptr::null_mut()), *hdc_screen)
		}) {
			Some(hdc_screen) => hdc_screen,
			None => return Err(os_err!("GetDC failed").into()),
		};

		let hdc = match UniquePtr::new_nullable(winapi::um::wingdi::CreateCompatibleDC(*hdc_screen), |hdc| {
			winapi::um::wingdi::DeleteDC(*hdc)
		}) {
			Some(hdc) => hdc,
			None => return Err(os_err!("CreateCompatibleDC failed").into()),
		};

		let hbmp = match UniquePtr::new_nullable(winapi::um::wingdi::CreateCompatibleBitmap(*hdc_screen, bounds.w, bounds.h), |hbmp| {
			winapi::um::wingdi::DeleteObject(*hbmp as *mut _)
		}) {
			Some(hbmp) => hbmp,
			None => return Err(os_err!("CreateCompatibleBitmap failed").into()),
		};

		let so = winapi::um::wingdi::SelectObject(*hdc, *hbmp as winapi::shared::windef::HGDIOBJ);
		if so == winapi::um::wingdi::HGDI_ERROR || so.is_null() {
			return Err(os_err!("SelectObject failed").into());
		}

		if let Some(clip) = clip {
			let vertices = [
				winapi::shared::windef::POINT { x: 0, y: 0 },
				winapi::shared::windef::POINT { x: clip.w, y: 0 },
				winapi::shared::windef::POINT { x: 0, y: clip.h },
			];
			if winapi::um::wingdi::PlgBlt(
				*hdc,
				vertices.as_ptr(),
				*hdc_screen,
				clip.x,
				clip.y,
				clip.w,
				clip.h,
				core::ptr::null_mut(),
				0,
				0,
			) == 0
			{
				return Err(os_err!("PlgBlt failed").into());
			}
		}

		if let Some(window_handle) = window_handle {
			let pw = winapi::um::winuser::PrintWindow(window_handle, *hdc, winapi::um::winuser::PW_CLIENTONLY);
			if pw == 0 {
				return Err(os_err!("PrintWindow failed").into());
			}
		}

		let (w, h) = match clip {
			Some(clip) => (clip.w, clip.h),
			None => (bounds.w, bounds.h),
		};

		let bmih = winapi::um::wingdi::BITMAPINFOHEADER {
			biSize: core::mem::size_of::<winapi::um::wingdi::BITMAPINFOHEADER>() as u32,
			biPlanes: 1,
			biBitCount: 32,
			biWidth: w,
			biHeight: -h,
			biCompression: winapi::um::wingdi::BI_RGB,
			biSizeImage: 0,
			biXPelsPerMeter: 0,
			biYPelsPerMeter: 0,
			biClrUsed: 0,
			biClrImportant: 0,
		};

		let mut bmi = winapi::um::wingdi::BITMAPINFO {
			bmiHeader: bmih,
			..Default::default()
		};

		let buffer_len = 4 * w as usize * h as usize;
		let mut buffer = Vec::with_capacity(buffer_len);

		let gdb = winapi::um::wingdi::GetDIBits(
			*hdc,
			*hbmp,
			0,
			h as u32,
			buffer.as_mut_ptr() as *mut _,
			&mut bmi,
			winapi::um::wingdi::DIB_RGB_COLORS,
		);
		if gdb == winapi::shared::winerror::ERROR_INVALID_PARAMETER as i32 {
			return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid parameter").into());
		} else if gdb == 0 {
			return Err(os_err!("GetDIBits failed").into());
		}

		buffer.set_len(buffer_len);

		// Get rid of the alpha channel
		for i in (0..buffer_len).step_by(4) {
			buffer[i + 3] = 255;
		}

		let image = image::ImageBuffer::from_raw(w as u32, h as u32, buffer.into_boxed_slice()).sus_unwrap();

		// FIXME -dx12 shows black screen if we capture the window
		if Blackout::update(&image, raw_window) {
			frame()
		} else {
			Ok(image)
		}
	}
}

#[test]
fn test_capture_squad() {
	let image = frame().unwrap();
	let image: image::RgbaImage = image.convert();
	open_image!(image);
}
