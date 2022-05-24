use smh_util::image;

#[derive(Clone, Copy, Debug)]
pub enum CornerBoundX {
	Left(RelativeBound),
	Right(RelativeBound)
}
impl CornerBoundX {
	#[inline]
	pub fn into_absolute(self, screen_size: [u32; 2], w: u32) -> u32 {
		match self {
			Self::Left(left) => left.into_absolute(screen_size),
			Self::Right(right) => screen_size[0] - right.into_absolute(screen_size) - w
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub enum CornerBoundY {
	Top(RelativeBound),
	Bottom(RelativeBound)
}
impl CornerBoundY {
	#[inline]
	pub fn into_absolute(self, screen_size: [u32; 2], h: u32) -> u32 {
		match self {
			Self::Top(top) => top.into_absolute(screen_size),
			Self::Bottom(bottom) => screen_size[1] - bottom.into_absolute(screen_size) - h
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct RelativeBounds2D {
	pub x: CornerBoundX,
	pub y: CornerBoundY,
	pub w: RelativeBound,
	pub h: RelativeBound
}
impl RelativeBounds2D {
	#[inline]
	pub fn into_absolute(self, screen_size: [u32; 2]) -> [u32; 4] {
		let (w, h) = (self.w.into_absolute(screen_size), self.h.into_absolute(screen_size));
		let (x, y) = (self.x.into_absolute(screen_size, w), self.y.into_absolute(screen_size, h));
		[x, y, w, h]
	}

	pub fn view<I: image::GenericImageView>(self, image: &I) -> image::SubImage<&<I as image::GenericImageView>::InnerImageView> {
		let [x, y, w, h] = self.into_absolute([image.width(), image.height()]);
		image.view(x, y, w, h)
	}
}

#[derive(Clone, Copy, Debug)]
pub enum RelativeBound {
	ScreenW(f64),
	ScreenH(f64)
}
impl RelativeBound {
	#[inline]
	pub fn into_absolute(self, screen_size: [u32; 2]) -> u32 {
		match self {
			Self::ScreenW(w) => (w * screen_size[0] as f64).round() as u32,
			Self::ScreenH(h) => (h * screen_size[1] as f64).round() as u32
		}
	}
}