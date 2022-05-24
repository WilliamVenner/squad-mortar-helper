use crate::prelude::*;
use std::sync::atomic::AtomicBool;

static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
	DEBUG.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn on() {
	if !DEBUG.swap(true, std::sync::atomic::Ordering::AcqRel) {
		log::info!("Debug mode on");
	}
}

pub fn off() {
	if DEBUG.swap(false, std::sync::atomic::Ordering::AcqRel) {
		log::info!("Debug mode off");
	}
}

macro_rules! timeshares {
	{$($event:ident => $color:expr),*} => {
		#[derive(Default, Debug)]
		pub struct Timeshares {
			pub entire_frame: Option<Duration>,
			$(pub $event: Option<Duration>),*
		}
		impl Timeshares {
			pub fn iter(&self) -> impl Iterator<Item = (&'static str, &'static [f32; 3], Duration)> + '_ {
				[$((stringify!($event), &$color, &self.$event)),*].into_iter().filter_map(|(name, color, event)| event.as_ref().map(|event| (name, color, *event)))
			}
		}
	};
}
timeshares! {
	load_frame => [0.0, 1.0, 1.0],
	crop_to_map => [1.0, 0.4, 0.0],
	find_minimap => [0.0, 0.0, 1.0],
	ocr_preprocess => [0.0, 0.35, 1.0],
	ocr => [0.35, 0.0, 1.0],
	find_scales_preprocess => [1.0, 0.0, 1.0],
	calc_meters_to_px_ratio => [1.0, 0.0, 0.4],
	isolate_map_markers => [0.0, 1.0, 0.0],
	filter_map_marker_icons => [1.0, 0.65, 0.0],
	mask_marker_lines => [1.0, 1.0, 0.0],
	find_marker_lines => [1.0, 0.0, 0.0]
}

static DEBUG_VIEW: AtomicU8 = AtomicU8::new(DebugView::None as u8);

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugView {
	None = 0,
	OCRInput,
	FindScalesInput,
	LSDPreprocess,
	LSDInput,
}
impl DebugView {
	pub fn get() -> Self {
		DebugView::try_from(DEBUG_VIEW.load(std::sync::atomic::Ordering::Acquire)).sus_unwrap()
	}

	pub fn set(value: Self) {
		DEBUG_VIEW.store(value as u8, std::sync::atomic::Ordering::Release);
	}

	pub fn variants() -> impl Iterator<Item = (&'static str, Self)> {
		[
			("OCR", Self::OCRInput),
			("Scales", Self::FindScalesInput),
			("Marker Isolation", Self::LSDPreprocess),
			("Line Segment Detection", Self::LSDInput),
		]
		.into_iter()
	}
}
impl TryFrom<u8> for DebugView {
	type Error = u8;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(DebugView::None),
			1 => Ok(DebugView::OCRInput),
			2 => Ok(DebugView::FindScalesInput),
			3 => Ok(DebugView::LSDPreprocess),
			4 => Ok(DebugView::LSDInput),
			_ => Err(value),
		}
	}
}
impl Default for DebugView {
	#[inline]
	fn default() -> Self {
		Self::None
	}
}

#[derive(Debug)]
pub enum DebugViewImage {
	None,
	Requested(DebugView),
	Some(Arc<image::RgbaImage>),
}
impl DebugViewImage {
	#[inline]
	pub fn get_image(&self) -> Option<&Arc<image::RgbaImage>> {
		match self {
			DebugViewImage::None | DebugViewImage::Requested(_) => None,
			DebugViewImage::Some(image) => Some(image),
		}
	}
}
impl Default for DebugViewImage {
	#[inline]
	fn default() -> Self {
		match DebugView::get() {
			DebugView::None => Self::None,
			view => Self::Requested(view),
		}
	}
}

#[derive(Debug)]
struct DebugActive(bool);
impl Default for DebugActive {
	fn default() -> Self {
		Self(DEBUG.load(std::sync::atomic::Ordering::Relaxed))
	}
}

#[derive(Debug, Default)]
pub struct DebugBox {
	debug: DebugActive,

	pub dpi: Option<u32>,
	pub timeshares: Timeshares,
	pub ocr: Vec<smh_vision_ocr::OCRText>,
	pub scales: SmallVec<(u32, Line<u32>), 3>,
	pub debug_view: DebugViewImage,
}
impl DebugBox {
	#[inline]
	pub fn active(&self) -> bool {
		self.debug.0
	}
}