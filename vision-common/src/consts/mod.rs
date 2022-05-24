use crate::prelude::*;

#[allow(clippy::module_inception)]
mod consts;
pub use consts::*;

pub const MAP_BOUNDS: RelativeBounds2D = RelativeBounds2D {
	x: Left(ScreenH(0.018522135)),
	y: Bottom(ScreenH(0.07421875)),
	w: ScreenH(0.864930556), // Map fills remaining space
	h: ScreenH(0.761078559)
};

pub const CLOSE_DEPLOYMENT_BUTTON_BOUNDS: RelativeBounds2D = RelativeBounds2D {
	x: Right(ScreenH(0.0078125)),
	y: Bottom(ScreenH(0.0078125)),
	w: ScreenH(0.236132813),
	h: ScreenH(0.038205295)
};