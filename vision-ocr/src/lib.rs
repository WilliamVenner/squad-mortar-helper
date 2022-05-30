#[macro_use] extern crate lazy_static;

use smh_util::*;
use std::{ffi::{CStr, c_void}, os::raw::{c_int, c_char, c_uchar, c_float}, sync::atomic::AtomicBool};

// https://tesseract-ocr.github.io/tessapi/5.x/a02438.html

// Windows
// cmake .. -DGRAPHICS_DISABLED=ON -DDISABLED_LEGACY_ENGINE=ON -DBUILD_TRAINING_TOOLS=OFF -DDISABLE_CURL=ON -DDISABLE_ARCHIVE=ON -A x64
// cmake .. -DGRAPHICS_DISABLED=ON -DDISABLED_LEGACY_ENGINE=ON -DBUILD_TRAINING_TOOLS=OFF -DDISABLE_CURL=ON -DDISABLE_ARCHIVE=ON -A Win32
// cmake --build . --config Release --target libtesseract

// Linux
// export CC=clang
// export CXX=clang++
// ./autogen.sh

// 64-bit
// export CFLAGS="-fPIC"
// export CXXFLAGS="-fPIC"

// 32-bit
// export CFLAGS="-m32 -fPIC"
// export CXXFLAGS="-m32 -fPIC"

// ./configure --disable-graphics --disable-legacy --disable-doc --disable-openmp --without-curl --without-archive --without-tensorflow --enable-shared=no --enable-static=yes
// make

// tesseract.exe --oem 1 --tessdata-dir "C:\Users\William\Documents\GitHub\squad-ruler\src\vision\ocr" "C:/Users/William/BLACKHOLE/temp.png" - -c tessedit_write_images=1

#[link(name = "smh_ocr", kind = "static")]
extern "C" {
	fn smh_ocr_tesseract_version() -> *const c_char;
	fn smh_ocr_init(out: *mut *mut c_void, data: *const c_char, len: c_int, lang: *const c_char) -> i32;
	fn smh_ocr_destroy(tesseract: *mut c_void);
	fn smh_ocr_recognise(tesseract: *mut c_void, ppi: c_int, image: *const c_uchar, width: c_int, height: c_int, bytes_per_pixel: c_int, bytes_per_line: c_int);
	fn smh_ocr_iter(tesseract: *mut c_void, state: *mut c_void, iter_fn: extern "C" fn(*mut c_void, COcrResult));
}

#[repr(transparent)]
pub struct Tesseract(*mut c_void);
impl Tesseract {
	fn init(data: &[u8], lang: *const c_char) -> Self {
		let _rustc_please_link_to_leptonica_thank_you = leptonica_sys::LIBLEPT_MAJOR_VERSION;

		log::info!("Initializing Tesseract OCR version v{}", unsafe { CStr::from_ptr(smh_ocr_tesseract_version()) }.to_string_lossy());

		let mut ptr = std::ptr::null_mut();
		let res = unsafe { smh_ocr_init(&mut ptr, data.as_ptr() as *const _, data.len() as _, lang as *const _) };
		if res == 0 {
			Tesseract(ptr)
		} else {
			panic!("Failed to initialize Tesseract OCR! Error {res}");
		}
	}

	fn recognise(&mut self, image: &[u8], width: u32, height: u32, ppi: i32) -> SusRef<'static, [OCRText]> {
		static TEXT_BUFFER: SusRefCell<Vec<OCRText>> = SusRefCell::new(Vec::new());

		{
			let mut state = TEXT_BUFFER.borrow_mut();
			state.clear();

			unsafe {
				smh_ocr_recognise(self.0, ppi, image.as_ptr(), width as i32, height as i32, 1, width as i32);
				smh_ocr_iter(self.0, &mut *state as *mut Vec<OCRText> as *mut c_void, Self::iter_fn);
			}
		}

		SusRef::map(TEXT_BUFFER.borrow(), #[inline] |vec: &Vec<OCRText>| vec.as_slice())
	}

	extern "C" fn iter_fn(results: *mut c_void, result: COcrResult) {
		let results = unsafe { &mut *(results as *mut Vec<OCRText>) };

		// SAFETY: We get UTF-8 text from the other side
		let text = unsafe { std::str::from_utf8_unchecked(CStr::from_ptr(result.text).to_bytes()) }.trim();
		if text.is_empty() { return };

		results.push(OCRText {
			text: text.to_string(),
			confidence: result.confidence,
			left: result.left as u32,
			top: result.top as u32,
			right: result.right as u32,
			bottom: result.bottom as u32
		});
	}
}
impl Drop for Tesseract {
    fn drop(&mut self) {
		if self.0.is_null() { return };
        unsafe { smh_ocr_destroy(self.0) };
		self.0 = std::ptr::null_mut();
    }
}

#[repr(C)]
struct COcrResult {
	text: *const c_char,
	confidence: c_float,
	left: c_int,
	top: c_int,
	right: c_int,
	bottom: c_int
}

#[derive(Debug, Clone)]
pub struct OCRText {
	pub text: String,
	pub confidence: f32,

	pub left: u32,
	pub top: u32,
	pub right: u32,
	pub bottom: u32
}

macro_rules! cstr {
	($text:literal) => {
		#[allow(unused_unsafe)] unsafe { CStr::from_ptr(concat!($text, "\0").as_ptr() as *const std::os::raw::c_char) }
	};

	($text:expr) => {
		#[allow(unused_unsafe)] unsafe { CStr::from_ptr(concat!($text, "\0").as_ptr() as *const std::os::raw::c_char) }
	};
}

fn training_data() -> Result<Vec<u8>, std::io::Error> {
	use std::io::Read;

	static TRAINED_DATA: &[u8] = include_bytes!("../resources/eng.traineddata.tar.gz");

	let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(std::io::Cursor::new(TRAINED_DATA)));
	let mut entry = archive.entries()?.next().unwrap()?;

	let mut data = Vec::with_capacity(entry.size() as usize);
	entry.read_to_end(&mut data)?;

	Ok(data)
}

pub fn read(image: &[u8], w: u32, h: u32, dpi: Option<u32>) -> SusRef<'static, [OCRText]> {
	let dpi = dpi.map(|dpi| dpi as i32).unwrap_or(-1);
	(&*TESSERACT).borrow_mut().recognise(image, w, h, dpi)
}

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

lazy_static! {
	static ref TESSERACT: SusRefCell<Tesseract> = SusRefCell::new({
		if SHUTTING_DOWN.load(std::sync::atomic::Ordering::Acquire) {
			// null pointer is safe here because we check for it in the drop impl
			Tesseract(std::ptr::null_mut())
		} else {
			Tesseract::init(&training_data().expect("Failed to decode trained OCR data"), cstr!("eng").as_ptr())
		}
	});
}

pub fn shutdown() {
	log::info!("shutting down ocr...");

	SHUTTING_DOWN.store(true, std::sync::atomic::Ordering::Release);

	let mut tesseract = (&*TESSERACT).borrow_mut();
	unsafe { core::ptr::drop_in_place(&mut *tesseract) };
	core::mem::forget(tesseract);
}