use crate::*;

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
	pub pixel: image::Rgba<u8>
}

#[derive(Clone, Copy, Debug)]
pub struct MarkedMapMarkerPixel {
	pub visible: bool,
	pub pixel: image::Rgba<u8>
}

fn isolate_map_markers(image: &mut image::RgbaImage) {
	use crate::consts::*;

	// Isolate green pixels
	for pixel in image.pixels_mut() {
		let (h, s, v) = pixel.to_hsv();
		if h < FIND_MARKER_HSV_RANGE_HUE[0] || h > FIND_MARKER_HSV_RANGE_HUE[1] || s < FIND_MARKER_HSV_RANGE_SAT || v < FIND_MARKER_HSV_RANGE_VIB {
			*pixel = image::Rgba([0, 0, 0, 0]);
		}
	}
}

fn process_marker<P: MapMarkerFilter>(marker: &'static [u8], size: u32, corners: Option<&[MapMarkerPixel]>) -> Box<[P::Pixel]> {
	let marker = image::load_from_memory_with_format(marker, image::ImageFormat::Tga).expect("Failed to load embedded map marker TGA! This should never happen...");
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
	RAW_MARKERS.into_par_iter().map(|marker| process_marker::<P>(marker, size, Some(&corners))).collect::<Vec<_>>().try_into().unwrap()
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