use crate::prelude::*;

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