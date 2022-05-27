use image::Pixel;
use rayon::prelude::*;

use crate::{SusUnwrap, UnsafeSendPtr};

#[macro_export]
macro_rules! par_iter_pixels {
	($image:ident[$x:expr, $y:expr, $w:expr, $h:expr]) => {{
		if $x + $w > $image.width() || $y + $h > $image.height() {
			panic!(
				"iter_pixels ({}, {}) to ({}, {}) is outside of bounds for {}x{} image",
				$x, $y, ($x + $w) - 1, ($y + $h) - 1,
				$image.width(),
				$image.height()
			);
		}

		let self_ = $crate::UnsafeSendPtr::new_const(&$image);
		($x..($x + $w)).into_par_iter().map(move |x| ($y..($y + $h)).into_par_iter().map(move |y| (x, y))).flatten().map(move |(x_, y_)| {
			let self_ = unsafe { self_.as_const() };

			#[cfg(debug_assertions)]
			let p = self_.get_pixel(x_, y_);

			#[cfg(not(debug_assertions))]
			let p = unsafe { self_.unsafe_get_pixel(x_, y_) };

			(x_, y_, p)
		})
	}};

	($image:ident) => {{
		let (w, h) = $image.dimensions();
		let self_ = $crate::UnsafeSendPtr::new_const(&$image);
		(0..w).into_par_iter().map(move |x| (0..h).into_par_iter().map(move |y| (x, y))).flatten().map(move |(x_, y_)| {
			let self_ = unsafe { self_.as_const() };

			#[cfg(debug_assertions)]
			let p = self_.get_pixel(x_, y_);

			#[cfg(not(debug_assertions))]
			let p = unsafe { self_.unsafe_get_pixel(x_, y_) };

			(x_, y_, p)
		})
	}};
}

#[macro_export]
macro_rules! iter_pixels {
	($image:ident[$x:expr, $y:expr, $w:expr, $h:expr]) => {{
		if $x + $w > $image.width() || $y + $h > $image.height() {
			panic!(
				"iter_pixels ({}, {}) to ({}, {}) is outside of bounds for {}x{} image",
				$x, $y, ($x + $w) - 1, ($y + $h) - 1,
				$image.width(),
				$image.height()
			);
		}

		($x..($x + $w)).into_iter().map(move |x| ($y..($y + $h)).into_iter().map(move |y| (x, y))).flatten().map(|(x_, y_)| {
			#[cfg(debug_assertions)]
			let p = *$image.get_pixel(x_, y_);

			#[cfg(not(debug_assertions))]
			let p = unsafe { $image.unsafe_get_pixel(x_, y_) };

			(x_, y_, p)
		})
	}};

	($image:ident) => {{
		let (w, h) = $image.dimensions();
		(0..w).into_iter().map(move |x| (0..h).into_iter().map(move |y| (x, y))).flatten().map(|(x_, y_)| {
			#[cfg(debug_assertions)]
			let p = *$image.get_pixel(x_, y_);

			#[cfg(not(debug_assertions))]
			let p = unsafe { $image.unsafe_get_pixel(x_, y_) };

			(x_, y_, p)
		})
	}};
}

pub trait ParallelCrop: image::GenericImageView
where
	Self::Pixel: 'static
{
	fn par_crop<P: image::Pixel + 'static>(&self, x: u32, y: u32, w: u32, h: u32) -> image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>> where Self::Pixel: ConvertPixel<P>;
	fn par_crop_into<P: image::Pixel + 'static>(&self, x: u32, y: u32, w: u32, h: u32, cropped: &mut image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>>) where Self::Pixel: ConvertPixel<P>;
}
impl<I: image::GenericImageView> ParallelCrop for I
where
	I::Pixel: 'static
{
	fn par_crop<P: image::Pixel + 'static>(&self, x: u32, y: u32, w: u32, h: u32) -> image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>> where Self::Pixel: ConvertPixel<P> {
		let mut cropped = image::ImageBuffer::new(w, h);
		self.par_crop_into(x, y, w, h, &mut cropped);
		cropped
	}

	fn par_crop_into<P: image::Pixel + 'static>(&self, x: u32, y: u32, w: u32, h: u32, cropped: &mut image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>>) where Self::Pixel: ConvertPixel<P> {
		if x + w >= self.width() || y + h >= self.height() {
			panic!(
				"crop region ({x}, {y}) is outside of bounds for {}x{} image",
				self.width(),
				self.height()
			);
		}

		if w > cropped.width() || h > cropped.height() {
			panic!(
				"crop size ({w}, {h}) is too large for {}x{} image",
				cropped.width(),
				cropped.height()
			);
		}

		let par_image = UnsafeSendPtr::new_const(self);
		let par_cropped = UnsafeSendPtr::new_mut(cropped);
		(x..(x + w)).into_par_iter().for_each(
			|px_x| {
				(y..(y + h)).into_par_iter().for_each(
					|px_y| {
						let image = unsafe { par_image.clone().as_const() };
						let cropped = unsafe { par_cropped.clone().as_mut() };

						let (cropped_x, cropped_y) = (px_x - x, px_y - y);

						#[cfg(debug_assertions)]
						{
							if px_x == x && px_y == y {
								assert_eq!(cropped_x, 0);
								assert_eq!(cropped_y, 0);
							} else if px_x == x + w - 1 && px_y == y + h - 1 {
								assert_eq!(cropped_x, w - 1);
								assert_eq!(cropped_y, h - 1);
							}
						}

						cropped.put_pixel_fast(cropped_x, cropped_y, image.get_pixel(px_x, px_y).convert());
					},
				);
			},
		);
	}
}

pub trait GetPixelCheckedPolyfill: image::GenericImageView {
	fn get_pixel_checked(&self, x: u32, y: u32) -> Option<<Self as image::GenericImageView>::Pixel>;
}
impl<I: image::GenericImageView> GetPixelCheckedPolyfill for I {
	#[inline]
	fn get_pixel_checked(&self, x: u32, y: u32) -> Option<<Self as image::GenericImageView>::Pixel> {
		if x >= self.width() || y >= self.height() {
			return None;
		}
		unsafe { Some(self.unsafe_get_pixel(x, y)) }
	}
}

pub trait FastPixelGet: image::GenericImageView {
	fn get_pixel_fast(&self, x: u32, y: u32) -> <Self as image::GenericImageView>::Pixel;
}
impl<I: image::GenericImageView> FastPixelGet for I {
	#[inline]
	#[cfg(debug_assertions)]
	fn get_pixel_fast(&self, x: u32, y: u32) -> <Self as image::GenericImageView>::Pixel {
		self.get_pixel(x, y)
	}

	#[inline]
	#[cfg(not(debug_assertions))]
	fn get_pixel_fast(&self, x: u32, y: u32) -> <Self as image::GenericImageView>::Pixel {
		unsafe { self.unsafe_get_pixel(x, y) }
	}
}
pub trait FastPixelSet: image::GenericImageView + image::GenericImage {
	fn put_pixel_fast(&mut self, x: u32, y: u32, pixel: <Self as image::GenericImageView>::Pixel);
}
impl<I: image::GenericImageView + image::GenericImage> FastPixelSet for I {
	#[inline]
	#[cfg(debug_assertions)]
	fn put_pixel_fast(&mut self, x: u32, y: u32, pixel: <Self as image::GenericImageView>::Pixel) {
		self.put_pixel(x, y, pixel)
	}

	#[inline]
	#[cfg(not(debug_assertions))]
	fn put_pixel_fast(&mut self, x: u32, y: u32, pixel: <Self as image::GenericImageView>::Pixel) {
		unsafe { self.unsafe_put_pixel(x, y, pixel) }
	}
}

fn hsv(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
	let r = r as f32 / 255.0;
	let g = g as f32 / 255.0;
	let b = b as f32 / 255.0;

	let max = r.max(g.max(b));
	let min = r.min(g.min(b));
	let delta = max - min;

	let h = if max == min {
		0.0
	} else if max == r {
		60.0 * (((g - b) / delta) % 6.0)
	} else if max == g {
		60.0 * (((b - r) / delta) + 2.0)
	} else {
		60.0 * (((r - g) / delta) + 4.0)
	};
	let s = 100.0 * delta / max;
	let v = 100.0 * max;

	(h as u16, s as u8, v as u8)
}
pub trait HSV: image::Pixel {
	fn to_hsv(self) -> (u16, u8, u8);
}
impl HSV for image::Rgb<u8> {
	#[inline]
	fn to_hsv(self) -> (u16, u8, u8) {
		hsv(self.0[0], self.0[1], self.0[2])
	}
}
impl HSV for image::Rgba<u8> {
	#[inline]
	fn to_hsv(self) -> (u16, u8, u8) {
		hsv(self.0[0], self.0[1], self.0[2])
	}
}

pub trait AsRefImage<P: image::Pixel> {
	fn as_ref_image(&self) -> image::ImageBuffer<P, &[P::Subpixel]>;
}
impl<P, C> AsRefImage<P> for image::ImageBuffer<P, C>
where
	P: image::Pixel + 'static,
	C: core::ops::Deref<Target = [P::Subpixel]>
{
	#[inline]
	fn as_ref_image(&self) -> image::ImageBuffer<P, &[P::Subpixel]> {
		image::ImageBuffer::from_raw(self.width(), self.height(), self.as_raw().as_ref()).sus_unwrap()
	}
}

mod sub_image {
	use super::*;
	use image::GenericImageView;

	pub struct OwnedSubImageInner<I: image::GenericImageView + image::GenericImage>(I);
	impl<I: image::GenericImageView + image::GenericImage> core::ops::Deref for OwnedSubImageInner<I> {
		type Target = I;

		#[inline]
		fn deref(&self) -> &Self::Target {
			&self.0
		}
	}
	impl<I: image::GenericImageView + image::GenericImage> core::ops::DerefMut for OwnedSubImageInner<I> {
		#[inline]
		fn deref_mut(&mut self) -> &mut Self::Target {
			&mut self.0
		}
	}

	pub struct OwnedSubImage<I: image::GenericImageView + image::GenericImage>(image::SubImage<OwnedSubImageInner<I>>);
	impl<I: image::GenericImageView + image::GenericImage> OwnedSubImage<I> {
		#[inline]
		pub fn new(image: I, x: u32, y: u32, w: u32, h: u32) -> Self {
			Self(image::SubImage::new(OwnedSubImageInner(image), x, y, w, h))
		}

		#[inline]
		pub fn view(&self, x: u32, y: u32, w: u32, h: u32) -> image::SubImage<&I> {
			self.0.view(x, y, w, h)
		}

		/// Convert this subimage to an ImageBuffer with a converted pixel type
		pub fn to_converted_image<P: image::Pixel + Sized + Send + Sync + 'static>(&self) -> image::ImageBuffer<P, Vec<P::Subpixel>> where I::Pixel: Send + Sync + super::ConvertPixel<P> + 'static {
			debug_assert!(std::any::TypeId::of::<P>() != std::any::TypeId::of::<I::Pixel>(), "to_converted_image is not performant when the pixel types are the same");

			let mut out = image::ImageBuffer::new(self.width(), self.height());
			let par_out = UnsafeSendPtr::new_mut(&mut out);

			let subimage = &self.0;
			par_iter_pixels!(subimage).for_each(|(x, y, pixel)| {
				let out = unsafe { par_out.clone().as_mut() };
				let pixel = pixel.convert();
				out.put_pixel_fast(x, y, pixel);
			});

			out
		}
	}
	impl<I: image::GenericImageView + image::GenericImage> core::ops::Deref for OwnedSubImage<I> {
		type Target = image::SubImage<OwnedSubImageInner<I>>;

		#[inline]
		fn deref(&self) -> &Self::Target {
			&self.0
		}
	}
	impl<I: image::GenericImageView + image::GenericImage> core::ops::DerefMut for OwnedSubImage<I> {
		#[inline]
		fn deref_mut(&mut self) -> &mut Self::Target {
			&mut self.0
		}
	}
	impl<I: image::GenericImageView + image::GenericImage + Default> Default for OwnedSubImage<I> {
		#[inline]
		fn default() -> Self {
			Self(image::SubImage::new(OwnedSubImageInner(I::default()), 0, 0, 0, 0))
		}
	}
}
pub use sub_image::OwnedSubImage;

pub trait ConvertPixel<T: Sized + image::Pixel>: Sized + image::Pixel {
	fn convert(self) -> T;
}
impl<P: image::Primitive + 'static> ConvertPixel<image::Rgb<P>> for image::Bgra<P> {
	#[inline]
	fn convert(self) -> image::Rgb<P> {
		image::Rgb::from_channels(self.0[2], self.0[1], self.0[0], self.0[3])
	}
}