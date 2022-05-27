#![allow(clippy::missing_safety_doc)]

pub use smh_util::*;

pub mod prelude {
	pub use crate::{
		debug,
		screen::{
			CornerBoundX::{self, *},
			CornerBoundY::{self, *},
			RelativeBound::{self, *},
			RelativeBounds2D, *,
		},
		lsd, markers,
	};

	pub type VisionFrame = OwnedSubImage<image::ImageBuffer<image::Bgra<u8>, Box<[u8]>>>;

	pub use smh_util::*;
}
use prelude::*;

pub mod consts;
pub mod debug;
pub mod dylib;
pub mod screen;
pub mod lsd;
pub mod markers;

pub trait Vision: Sized + Send + Sync {
	type LSDImage;
	type Error: Send + Sync;

	/// Thread-local state
	///
	/// Needed for multithreading CUDA
	fn thread_ctx(&self) -> Result<(), AnyError>;
	fn init() -> Result<Self, AnyError>;

	/// Get an owned reference to the current CPU frame image
	///
	/// Needed for find_minimap
	fn get_cpu_frame(&self) -> Arc<VisionFrame>;

	fn load_frame(&mut self, image: VisionFrame) -> Result<(), Self::Error>;
	fn load_map_markers(&mut self, map_marker_size: u32) -> Result<(), Self::Error>;

	fn crop_to_map(&self, grayscale: bool) -> Result<Option<(image::RgbaImage, [u32; 4])>, Self::Error>;

	// TODO: might be able to replace pointer here with a GAT type when stabilized - passing lifetimes across the dylib boundary is not easy
	fn ocr_preprocess(&self) -> Result<(*const u8, usize), Self::Error>;
	fn find_scales_preprocess(&self, scales_start_y: u32) -> Result<*const SusRefCell<image::GrayImage>, Self::Error>;

	fn isolate_map_markers(&self) -> Result<(), Self::Error>;
	fn filter_map_marker_icons(&self) -> Result<(), Self::Error>;
	fn mask_marker_lines(&self) -> Result<(), Self::Error>;
	fn find_longest_line(&self, image: &Self::LSDImage, pt: Point<f32>, max_gap: f32) -> Result<(Line<f32>, f32), Self::Error>;
	fn find_marker_lines(&self, max_gap: u32) -> Result<SmallVec<Line<f32>, 32>, Self::Error>;

	fn get_debug_view(&self, choice: debug::DebugView) -> Option<Arc<image::RgbaImage>>;
}