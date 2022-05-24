#[repr(transparent)]
#[derive(Debug)]
/// Rayon helper
pub struct UnsafeSendPtr<T: ?Sized>(core::ptr::NonNull<T>);
unsafe impl<T> Send for UnsafeSendPtr<T> {}
unsafe impl<T> Sync for UnsafeSendPtr<T> {}
impl<T: ?Sized> Copy for UnsafeSendPtr<T> {}
impl<T: ?Sized> Clone for UnsafeSendPtr<T> {
	#[inline(always)]
	fn clone(&self) -> Self {
		Self(self.0)
	}
}
impl<T: ?Sized> UnsafeSendPtr<T> {
	#[inline]
	pub fn new_mut(ptr: &mut T) -> Self {
		UnsafeSendPtr(unsafe { core::ptr::NonNull::new_unchecked(ptr) })
	}

	#[inline]
	pub fn new_const(ptr: &T) -> Self {
		UnsafeSendPtr(unsafe { core::ptr::NonNull::new_unchecked(ptr as *const T as *mut T) })
	}

	#[inline]
	pub unsafe fn as_mut<'a>(&mut self) -> &'a mut T {
		&mut *core::cell::UnsafeCell::raw_get(self.0.as_ptr() as *const _)
	}

	#[inline]
	pub unsafe fn as_const<'a>(&self) -> &'a T {
		&*core::cell::UnsafeCell::raw_get(self.0.as_ptr() as *const _)
	}
}

/*#[macro_export]
macro_rules! rayon_join_all {
	//// EXPLICIT THREAD POOL ////
	($threads:expr => {}) => {};

	($threads:expr => {|| $task1:expr, || $task2:expr, $($tail:tt)*}) => {
		$threads.join(
			|| $task1,
			|| $crate::rayon_join_all!($threads => {|| $task2, $($tail)*})
		)
	};

	($threads:expr => {|| $task1:expr, $($tail:tt)*}) => {
		$task1
	};

	//// NO EXPLICIT THREAD POOL ////
	() => {};

	(|| $task1:expr, || $task2:expr, $($tail:tt)*) => {
		rayon::join(
			|| $task1,
			|| $crate::rayon_join_all!(|| $task2, $($tail)*)
		)
	};

	(|| $task1:expr, $($tail:tt)*) => {
		$task1
	};
}*/

#[macro_export]
macro_rules! rayon_join_all {
	(@null_expr $expr:expr) => {()};

	//// NO EXPLICIT THREAD POOL ////
	{$_: ident: || $first:expr, $($name:ident: || $task:expr,)*} => {{
		$(let mut $name = MaybeUninit::uninit();)*
		rayon::scope(|s| {
			$(s.spawn(|_| $name = MaybeUninit::new((|| $task)()));)*
		});
		((|| $first)(), $(unsafe { $name.assume_init() }),*)
	}};

	//// EXPLICIT THREAD POOL ////
	($threads:expr => {$_: ident: || $first:expr, $($name:ident: || $task:expr,)*}) => {{
		$(let mut $name = MaybeUninit::uninit();)*
		$threads.scope(|s| {
			$(s.spawn(|_| $name = MaybeUninit::new((|| $task)()));)*
		});
		((|| $first)(), $(unsafe { $name.assume_init() }),*)
	}};
}