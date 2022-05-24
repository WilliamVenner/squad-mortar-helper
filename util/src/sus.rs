/// AtomicRefCell in debug, SUS (no checks) in release

#[cfg(debug_assertions)]
mod debug {
	#[repr(transparent)]
	pub struct SusRef<'a, T: ?Sized>(atomic_refcell::AtomicRef<'a, T>);
	impl<T: ?Sized> core::ops::Deref for SusRef<'_, T> {
		type Target = T;

		#[inline(always)]
		fn deref(&self) -> &Self::Target {
			self.0.deref()
		}
	}
	impl<'a, T> SusRef<'a, T> {
		#[inline(always)]
		pub fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> SusRef<'a, U> {
			SusRef(atomic_refcell::AtomicRef::map(self.0, f))
		}
	}

	#[repr(transparent)]
	pub struct SusRefMut<'a, T: ?Sized>(atomic_refcell::AtomicRefMut<'a, T>);
	impl<T: ?Sized> core::ops::Deref for SusRefMut<'_, T> {
		type Target = T;

		#[inline(always)]
		fn deref(&self) -> &Self::Target {
			self.0.deref()
		}
	}
	impl<T> core::ops::DerefMut for SusRefMut<'_, T> {
		#[inline(always)]
		fn deref_mut(&mut self) -> &mut Self::Target {
			self.0.deref_mut()
		}
	}

	#[repr(transparent)]
	#[derive(Default)]
	pub struct SusRefCell<T>(atomic_refcell::AtomicRefCell<T>);
	impl<T> SusRefCell<T> {
		#[inline(always)]
		pub const fn new(value: T) -> Self {
			SusRefCell(atomic_refcell::AtomicRefCell::new(value))
		}

		#[inline(always)]
		pub fn borrow(&self) -> SusRef<'_, T> {
			SusRef(self.0.borrow())
		}

		#[inline(always)]
		pub fn borrow_mut(&self) -> SusRefMut<'_, T> {
			SusRefMut(self.0.borrow_mut())
		}

		#[inline(always)]
		pub fn get_mut(&mut self) -> &mut T {
			self.0.get_mut()
		}
	}
}
#[cfg(debug_assertions)]
pub use debug::*;

#[cfg(not(debug_assertions))]
mod release {
	#[repr(transparent)]
	pub struct SusRef<'a, T: ?Sized>(&'a T);
	impl<T: ?Sized> core::ops::Deref for SusRef<'_, T> {
		type Target = T;

		#[inline(always)]
		fn deref(&self) -> &Self::Target {
			self.0
		}
	}
	impl<'a, T> SusRef<'a, T> {
		#[inline(always)]
		pub fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> SusRef<'a, U> {
			SusRef(f(self.0))
		}
	}

	#[repr(transparent)]
	pub struct SusRefMut<'a, T: ?Sized>(&'a mut T);
	impl<T: ?Sized> core::ops::Deref for SusRefMut<'_, T> {
		type Target = T;

		#[inline(always)]
		fn deref(&self) -> &Self::Target {
			self.0
		}
	}
	impl<T> core::ops::DerefMut for SusRefMut<'_, T> {
		#[inline(always)]
		fn deref_mut(&mut self) -> &mut Self::Target {
			self.0
		}
	}

	#[repr(transparent)]
	#[derive(Default)]
	pub struct SusRefCell<T>(core::cell::UnsafeCell<T>);
	impl<T> SusRefCell<T> {
		#[inline(always)]
		pub const fn new(value: T) -> Self {
			Self(core::cell::UnsafeCell::new(value))
		}

		#[inline(always)]
		pub fn borrow(&self) -> SusRef<'_, T> {
			SusRef(unsafe { &*self.0.get() })
		}

		#[inline(always)]
		pub fn borrow_mut(&self) -> SusRefMut<'_, T> {
			SusRefMut(unsafe { &mut *self.0.get() })
		}

		#[inline(always)]
		pub fn get_mut(&mut self) -> &mut T {
			self.0.get_mut()
		}
	}
}
#[cfg(not(debug_assertions))]
pub use release::*;

unsafe impl<T> Send for SusRefCell<T> {}
unsafe impl<T> Sync for SusRefCell<T> {}

impl<T> From<T> for SusRefCell<T> {
	#[inline(always)]
	fn from(value: T) -> Self {
		SusRefCell::new(value)
	}
}

pub trait SusUnwrap<T>: Sized {
	fn sus_unwrap(self) -> T;
	fn sus_expect(self, msg: &str) -> T;
}
impl<T> SusUnwrap<T> for Option<T> {
	#[inline(always)]
	#[cfg(debug_assertions)]
	fn sus_unwrap(self) -> T {
		self.unwrap()
	}

	#[inline(always)]
	#[cfg(debug_assertions)]
	fn sus_expect(self, msg: &str) -> T {
		self.expect(msg)
	}

	#[inline(always)]
	#[cfg(not(debug_assertions))]
	fn sus_unwrap(self) -> T {
		unsafe { self.unwrap_unchecked() }
	}

	#[inline(always)]
	#[cfg(not(debug_assertions))]
	fn sus_expect(self, _msg: &str) -> T {
		self.sus_unwrap()
	}
}
impl<T, E: std::fmt::Debug> SusUnwrap<T> for Result<T, E> {
	#[inline(always)]
	#[cfg(debug_assertions)]
	fn sus_unwrap(self) -> T {
		self.unwrap()
	}

	#[inline(always)]
	#[cfg(debug_assertions)]
	fn sus_expect(self, msg: &str) -> T {
		self.expect(msg)
	}

	#[inline(always)]
	#[cfg(not(debug_assertions))]
	fn sus_unwrap(self) -> T {
		unsafe { self.unwrap_unchecked() }
	}

	#[inline(always)]
	#[cfg(not(debug_assertions))]
	fn sus_expect(self, _msg: &str) -> T {
		self.sus_unwrap()
	}
}