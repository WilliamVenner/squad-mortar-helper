use super::*;
use core::ops::*;

#[derive(Clone, Copy, Debug)]
pub struct Rect<T> {
	pub left: T,
	pub top: T,
	pub right: T,
	pub bottom: T,
}
impl<T: Copy> Rect<T> {
	#[inline]
	pub fn top_left(&self) -> [T; 2] {
		[self.left, self.top]
	}

	#[inline]
	pub fn top_right(&self) -> [T; 2] {
		[self.right, self.top]
	}

	#[inline]
	pub fn bottom_left(&self) -> [T; 2] {
		[self.left, self.bottom]
	}

	#[inline]
	pub fn bottom_right(&self) -> [T; 2] {
		[self.right, self.bottom]
	}

	#[inline]
	pub fn width(&self) -> T
	where
		T: core::ops::Sub<Output = T>
	{
		self.right - self.left
	}

	#[inline]
	pub fn height(&self) -> T
	where
		T: core::ops::Sub<Output = T>
	{
		self.bottom - self.top
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Point<T> {
	pub x: T,
	pub y: T,
}
impl<T> Point<T> {
	#[inline]
	pub const fn new(x: T, y: T) -> Self {
		Self { x, y }
	}

	#[inline]
	pub fn distance_sqr(&self, other: &Self) -> f32
	where
		T: Sub<T, Output = T> + Mul<T, Output = T> + std::ops::Add<T, Output = T> + Copy + LossyFrom<f32>,
		f32: LossyFrom<T>
	{
		f32::lossy_from((self.x - other.x) * (self.x - other.x) + (self.y - other.y) * (self.y - other.y))
	}
}
impl<T> From<Point<T>> for [T; 2] {
	#[inline]
	fn from(pt: Point<T>) -> Self {
		[pt.x, pt.y]
	}
}
impl<T: Copy> From<[T; 2]> for Point<T> {
	#[inline]
	fn from(pt: [T; 2]) -> Self {
		Point { x: pt[0], y: pt[1] }
	}
}
impl<T> From<Point<T>> for (T, T) {
	#[inline]
	fn from(pt: Point<T>) -> Self {
		(pt.x, pt.y)
	}
}
impl<T> From<(T, T)> for Point<T> {
	#[inline]
	fn from((x, y): (T, T)) -> Self {
		Point { x, y }
	}
}
impl<T: Sub<T, Output = T>> Sub for Point<T> {
	type Output = Point<T>;

	#[inline]
	fn sub(self, rhs: Self) -> Self::Output {
		Point::new(
			self.x - rhs.x,
			self.y - rhs.y
		)
	}
}
impl<T: Mul<T, Output = T>> Mul for Point<T> {
	type Output = Point<T>;

	#[inline]
	fn mul(self, rhs: Self) -> Self::Output {
		Point::new(
			self.x * rhs.x,
			self.y * rhs.y
		)
	}
}
impl<T: Mul<T, Output = T> + Copy> Mul<T> for Point<T> {
	type Output = Point<T>;

	#[inline]
	fn mul(self, rhs: T) -> Self::Output {
		Point::new(
			self.x * rhs,
			self.y * rhs
		)
	}
}
impl<T: Div<T, Output = T>> Div for Point<T> {
	type Output = Point<T>;

	#[inline]
	fn div(self, rhs: Self) -> Self::Output {
		Point::new(
			self.x / rhs.x,
			self.y / rhs.y
		)
	}
}
impl<T: Div<T, Output = T> + Copy> Div<T> for Point<T> {
	type Output = Point<T>;

	#[inline]
	fn div(self, rhs: T) -> Self::Output {
		Point::new(
			self.x / rhs,
			self.y / rhs
		)
	}
}
impl<T: Add<T, Output = T>> Add for Point<T> {
	type Output = Point<T>;

	#[inline]
	fn add(self, rhs: Self) -> Self::Output {
		Point::new(
			self.x + rhs.x,
			self.y + rhs.y
		)
	}
}
impl<T: AddAssign<T>> AddAssign for Point<T> {
	#[inline]
	fn add_assign(&mut self, rhs: Self) {
		self.x += rhs.x;
		self.y += rhs.y;
	}
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Line<T> {
	pub p0: Point<T>,
	pub p1: Point<T>,
}
impl<T> Line<T> {
	#[inline]
	pub const fn new(p0: Point<T>, p1: Point<T>) -> Self {
		Self { p0, p1 }
	}
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct BBox<T> {
	pub x: T,
	pub y: T,
	pub w: T,
	pub h: T
}