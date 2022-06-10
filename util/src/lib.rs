#![allow(clippy::missing_safety_doc)]

pub use image::{buffer::ConvertBuffer, DynamicImage, EncodableLayout, GenericImage, GenericImageView, GrayImage, RgbImage};
pub use parking_lot::{Mutex, RwLock, Condvar};
pub use rayon::prelude::*;
pub use atomic_refcell::{AtomicRefCell, AtomicRef, AtomicRefMut};

pub type AnyError = anyhow::Error;

pub use std::{
	borrow::Cow,
	collections::{btree_map::Entry as BTreeMapEntry, BTreeMap, BTreeSet},
	ffi::{c_void, CStr, CString},
	fs::File,
	os::raw::{c_char, c_float, c_int, c_uchar},
	path::{Path, PathBuf},
	rc::Rc,
	sync::{
		atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, AtomicUsize},
		Arc,
	},
	thread::JoinHandle,
	time::{Instant, SystemTime},
};

pub use core::{
	borrow::{Borrow, BorrowMut},
	cell::{RefCell, UnsafeCell},
	marker::PhantomData,
	ops::{Deref, DerefMut},
	time::Duration,
	mem::MaybeUninit
};

pub use crossbeam_channel as crossbeam;
pub use rayon;
pub use image;
pub use imageproc;
pub use open;
pub use paste;
pub use parking_lot;
pub use atomic_refcell;
pub use anyhow;
pub use log;
pub use async_channel;
pub use chrono;
pub use byteorder;

mod sus;
pub use sus::*;

mod geometry;
pub use geometry::*;

mod debug;
pub use debug::*;

mod cell;
pub use cell::*;

mod maths;
pub use maths::*;

mod parallel;
pub use parallel::*;

mod smallvec;
pub use smallvec::*;

mod str;
pub use crate::str::*;

#[path = "image.rs"]
mod util_image;
pub use util_image::*;

pub trait LossyFrom<T>: Sized {
	fn lossy_from(val: T) -> Self;
}
impl<T> LossyFrom<T> for T {
	#[inline]
	fn lossy_from(val: T) -> Self {
		val
	}
}

pub trait LossyInto<T>: Sized {
	fn lossy_into(self) -> T;
}
impl<T: LossyFrom<U>, U> LossyInto<T> for U {
	#[inline]
	fn lossy_into(self) -> T {
		LossyFrom::lossy_from(self)
	}
}

macro_rules! impl_lossy_from {
	($($ty1:ty as $ty2:ty),*) => {$(
		impl LossyFrom<$ty1> for $ty2 {
			#[inline(always)]
			fn lossy_from(val: $ty1) -> Self {
				val as $ty2
			}
		}
		impl LossyFrom<$ty2> for $ty1 {
			#[inline(always)]
			fn lossy_from(val: $ty2) -> Self {
				val as $ty1
			}
		}
	)*}
}
impl_lossy_from!(
	i32 as f32,
	u32 as f32
);

pub trait FromBytesSlice {
	/// # Panics
	///
	/// Panics if the slice is not the same length as the size of the type.
	fn from_le_bytes_slice(slice: &[u8]) -> Self;

	/// # Panics
	///
	/// Panics if the slice is not the same length as the size of the type.
	fn from_be_bytes_slice(slice: &[u8]) -> Self;
}
macro_rules! impl_from_bytes_slice {
	($($ty:ty),*) => {
		$(impl FromBytesSlice for $ty {
			#[inline]
			fn from_le_bytes_slice(slice: &[u8]) -> Self {
				let mut bytes = [0u8; core::mem::size_of::<$ty>()];
				bytes.copy_from_slice(slice);
				<$ty>::from_le_bytes(bytes)
			}

			#[inline]
			fn from_be_bytes_slice(slice: &[u8]) -> Self {
				let mut bytes = [0u8; core::mem::size_of::<$ty>()];
				bytes.copy_from_slice(slice);
				<$ty>::from_be_bytes(bytes)
			}
		})*
	};
}
impl_from_bytes_slice!(u16, i16, u32, i32, u64, i64, u128, i128, f32, f64);

#[doc(hidden)]
pub struct UniquePtr<T, F: Fn(&mut T) -> R, R = ()> {
	value: T,
	destructor: F
}
impl<T, F, R> UniquePtr<T, F, R>
where
	F: Fn(&mut T) -> R
{
	#[inline]
	pub fn new(value: T, destructor: F) -> Self {
		UniquePtr { value, destructor }
	}
}
impl<T, F, R> UniquePtr<*mut T, F, R>
where
	F: Fn(&mut *mut T) -> R
{
	#[inline]
	pub fn new_nullable(value: *mut T, destructor: F) -> Option<Self> {
		if value.is_null() {
			None
		} else {
			Some(UniquePtr { value, destructor })
		}
	}
}
impl<T, F, R> Drop for UniquePtr<T, F, R>
where
	F: Fn(&mut T) -> R
{
	#[inline]
	fn drop(&mut self) {
		(self.destructor)(&mut self.value);
	}
}
impl<T, F, R> Deref for UniquePtr<T, F, R>
where
	F: Fn(&mut T) -> R
{
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.value
	}
}
impl<T, F, R> DerefMut for UniquePtr<T, F, R>
where
	F: Fn(&mut T) -> R
{
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}
impl<T, F, R> core::fmt::Debug for UniquePtr<T, F, R>
where
	T: core::fmt::Debug,
	F: Fn(&mut T) -> R
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UniquePtr").field("value", &self.value).finish()
    }
}