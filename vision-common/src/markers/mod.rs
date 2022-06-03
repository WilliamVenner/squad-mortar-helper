use crate::{*, consts::*};

const ALPHA_MARKER_COLOR_HSV_TUP: (u16, u8, u8) = (ALPHA_MARKER_COLOR_HSV[0], ALPHA_MARKER_COLOR_HSV[1] as _, ALPHA_MARKER_COLOR_HSV[2] as _);
const BRAVO_MARKER_COLOR_HSV_TUP: (u16, u8, u8) = (BRAVO_MARKER_COLOR_HSV[0], BRAVO_MARKER_COLOR_HSV[1] as _, BRAVO_MARKER_COLOR_HSV[2] as _);
const CHARLIE_MARKER_COLOR_HSV_TUP: (u16, u8, u8) = (
	CHARLIE_MARKER_COLOR_HSV[0],
	CHARLIE_MARKER_COLOR_HSV[1] as _,
	CHARLIE_MARKER_COLOR_HSV[2] as _,
);

macro_rules! markers {
	($($path:literal),*) => {
		const RAW_MARKERS: &[&[u8]] = &[$(include_bytes!($path)),*];
		pub const AMOUNT: usize = RAW_MARKERS.len();
	};
}
markers! {
	"resources/map_commandmarker_squad_attack.TGA",
	"resources/map_commandmarker_squad_build.TGA",
	"resources/map_commandmarker_squad_defend.TGA",
	"resources/map_commandmarker_squad_move.TGA",
	"resources/map_commandmarker_squad_observe.TGA"
}

#[derive(Clone, Copy, Debug)]
pub struct MapMarkerPixel {
	pub x: u32,
	pub y: u32,
	pub pixel: image::Rgba<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct MarkedMapMarkerPixel {
	pub visible: bool,
	pub pixel: image::Rgba<u8>,
}

/// Saturation is a special case.
///
/// The markers can be brightened by the lightness arc that the player icon emits on the map.
///
/// Therefore, we need some special logic to detect this whilst filtering out noise.
#[inline]
fn saturation_ok(s: u8, ms: u8) -> bool {
	ms.abs_diff(s) <= FIND_MARKER_HSV_SAT_TOLERANCE || (s as i16 - (ms as i16 - FIND_MARKER_PLAYER_DIR_ARC_SAT)).abs() as u8 <= FIND_MARKER_HSV_SAT_TOLERANCE
}

pub enum Fireteam {
	Alpha,
	Bravo,
	Charlie
}
pub fn debug_is_map_marker_color(h: u16, s: u8, v: u8, fireteam: Fireteam) -> [bool; 3] {
	let (mh, ms, mv) = match fireteam {
		Fireteam::Alpha => ALPHA_MARKER_COLOR_HSV_TUP,
		Fireteam::Bravo => BRAVO_MARKER_COLOR_HSV_TUP,
		Fireteam::Charlie => CHARLIE_MARKER_COLOR_HSV_TUP
	};

	[
		mh.abs_diff(h) <= FIND_MARKER_HSV_HUE_TOLERANCE,
		s >= FIND_MARKER_HSV_MIN_SAT && saturation_ok(s, ms),
		mv.abs_diff(v) <= FIND_MARKER_HSV_VIB_TOLERANCE
	]
}

pub fn is_any_map_marker_color<P: HSV>(pixel: P) -> bool {
	let (h, s, v) = pixel.to_hsv();

	if s < FIND_MARKER_HSV_MIN_SAT {
		return false;
	}

	[ALPHA_MARKER_COLOR_HSV_TUP, BRAVO_MARKER_COLOR_HSV_TUP, CHARLIE_MARKER_COLOR_HSV_TUP]
		.into_iter()
		.any(|(mh, ms, mv)| {
			mh.abs_diff(h) <= FIND_MARKER_HSV_HUE_TOLERANCE
				&& saturation_ok(s, ms)
				&& mv.abs_diff(v) <= FIND_MARKER_HSV_VIB_TOLERANCE
		})
}

fn isolate_map_markers(image: &mut image::RgbaImage) {
	image
		.pixels_mut()
		.filter(|pixel| !is_any_map_marker_color(**pixel))
		.for_each(|pixel| *pixel = image::Rgba([0, 0, 0, 0]))
}

fn process_marker<P: MapMarkerFilter>(marker: &'static [u8], size: u32, corners: Option<&[MapMarkerPixel]>) -> Box<[P::Pixel]> {
	let marker = image::load_from_memory_with_format(marker, image::ImageFormat::Tga)
		.expect("Failed to load embedded map marker TGA! This should never happen...");
	let marker = marker.resize_exact(size, size, image::imageops::FilterType::Gaussian);
	let mut marker = marker.into_rgba8();

	isolate_map_markers(&mut marker);

	if let Some(corners) = corners {
		for MapMarkerPixel { x, y, .. } in corners.iter().copied() {
			marker.put_pixel_fast(x, y, image::Rgba([0, 0, 0, 0]));
		}
	}

	P::apply(marker)
}

pub fn load_markers<P: MapMarkerFilter>(size: u32) -> [Box<[P::Pixel]>; AMOUNT] {
	let corners = process_marker::<FilteredMarkers>(include_bytes!("resources/corners.tga").as_slice(), size, None);
	RAW_MARKERS
		.into_par_iter()
		.map(|marker| process_marker::<P>(marker, size, Some(&corners)))
		.collect::<Vec<_>>()
		.try_into()
		.unwrap()
}

/// Allows us to specify if we want to filter out corner pixels, or mark them as invisible
pub trait MapMarkerFilter: Send + Sync {
	type Pixel: Send + Sync + std::fmt::Debug;
	fn apply(marker: image::RgbaImage) -> Box<[Self::Pixel]>;
}

/// Filters out corner pixels and alpha == 0
pub struct FilteredMarkers;
impl MapMarkerFilter for FilteredMarkers {
	type Pixel = MapMarkerPixel;

	#[inline]
	fn apply(marker: image::RgbaImage) -> Box<[Self::Pixel]> {
		marker
			.enumerate_pixels()
			.filter(|(_, _, pixel)| pixel.0[3] != 0)
			.map(|(x, y, pixel)| MapMarkerPixel { x, y, pixel: *pixel })
			.collect()
	}
}

/// Doesn't filter out any pixels
pub struct UnfilteredMarkers;
impl MapMarkerFilter for UnfilteredMarkers {
	type Pixel = image::Rgba<u8>;

	#[inline]
	fn apply(marker: image::RgbaImage) -> Box<[Self::Pixel]> {
		marker.pixels().copied().collect()
	}
}
