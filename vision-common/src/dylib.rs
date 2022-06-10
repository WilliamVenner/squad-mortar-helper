use super::*;

macro_rules! vision_dylib_wrapper {
	{
		Self => {$(fn $fn:ident($($arg:ident: $ty:ty),*)$( -> $ret:ty)?;)*};
		&mut self => {$(fn $mut_self_fn:ident(&mut self $(, $mut_self_arg:ident: $mut_self_ty:ty)*)$( -> $mut_self_ret:ty)?;)*};
		&self => {$(fn $self_fn:ident(&self $(, $self_arg:ident: $self_ty:ty)*)$( -> $self_ret:ty)?;)*};
	} => {
		pub struct VisionDylibWrapper<LSDImage, E> {
			shutdown: extern "Rust" fn(),
			$($mut_self_fn: extern "Rust" fn($($mut_self_arg: $mut_self_ty),*)$( -> $mut_self_ret)?,)*
			$($self_fn: extern "Rust" fn($($self_arg: $self_ty),*)$( -> $self_ret)?,)*
		}
		impl<LSDImage, E> VisionDylibWrapper<LSDImage, E> {
			pub fn wrap(name: &str) -> Result<Self, AnyError> {
				unsafe {
					let library = Box::leak(Box::new(libloading::Library::new(name).or_else(|_| libloading::Library::new(format!("deps/{name}")))?));

					(library.get::<extern "Rust" fn() -> Result<(), AnyError>>(format!("{name}_init\0").as_bytes()).expect("Could not find init function in vision dylib"))()?;

					Ok(Self {
						shutdown: *library.get(format!("{name}_shutdown\0").as_bytes()).expect("Could not find shutdown function in vision dylib"),
						$($mut_self_fn: *library.get(format!(concat!("{}_", stringify!($mut_self_fn), "\0"), name).as_bytes()).expect(concat!("Could not find function \"", stringify!($mut_self_fn), "\" in vision dylib")),)*
						$($self_fn: *library.get(format!(concat!("{}_", stringify!($self_fn), "\0"), name).as_bytes()).expect(concat!("Could not find function \"", stringify!($self_fn), "\" in vision dylib")),)*
					})
				}
			}
		}
		impl<LSDImage, E> Drop for VisionDylibWrapper<LSDImage, E> {
			#[inline]
			fn drop(&mut self) {
				(self.shutdown)();
			}
		}
		impl<LSDImage, E: Send + Sync> super::Vision for VisionDylibWrapper<LSDImage, E> {
			type LSDImage = LSDImage;
			type Error = E;

			#[inline]
			fn init() -> Result<Self, AnyError> {
				unimplemented!("VisionDylibWrapper::init()")
			}

			$(
				#[inline]
				fn $mut_self_fn(&mut self, $($mut_self_arg: $mut_self_ty),*)$( -> $mut_self_ret)? {
					(self.$mut_self_fn)($($mut_self_arg),*)
				}
			)*

			$(
				#[inline]
				fn $self_fn(&self, $($self_arg: $self_ty),*)$( -> $self_ret)? {
					(self.$self_fn)($($self_arg),*)
				}
			)*

			$(
				#[inline(always)]
				#[allow(unused_variables)]
				fn $fn($($arg: $ty),*)$( -> $ret)? {
					unimplemented!("cant call static function from VisionDylibWrapper")
				}
			)*
		}

		#[macro_export]
		macro_rules! export_dylib_wrapper {
			($name:ident => $state:ty) => {
				#[doc(hidden)]
				mod dylib_wrapper {
					use super::*;
					use std::{cell::UnsafeCell, mem::MaybeUninit, sync::atomic::AtomicBool};

					static mut STATE: UnsafeCell<MaybeUninit<$state>> = UnsafeCell::new(MaybeUninit::uninit());

					$crate::paste::paste! {
						#[no_mangle]
						pub extern "Rust" fn [<$name _ init>]() -> Result<(), AnyError> {
							log::info!(concat!("initializing external vision provider ", stringify!($state), "..."));
							unsafe { (&mut *STATE.get()).as_mut_ptr().write(<$state as $crate::Vision>::init()?) };
							log::info!(concat!(stringify!($state), " ready"));
							Ok(())
						}

						#[no_mangle]
						pub extern "Rust" fn [<$name _ shutdown>]() {
							static SHUTDOWN: AtomicBool = AtomicBool::new(false);
							if !SHUTDOWN.swap(true, std::sync::atomic::Ordering::SeqCst) {
								log::info!(concat!("shutting down external vision provider ", stringify!($state), "..."));
								unsafe { core::ptr::drop_in_place((&mut *STATE.get()).as_mut_ptr()) };
							}
						}
					}

					type E = <$state as $crate::Vision>::Error;
					type LSDImage = <$state as $crate::Vision>::LSDImage;

					$(
						$crate::paste::paste! {
							#[no_mangle]
							pub extern "Rust" fn [<$name _ $mut_self_fn>]($($mut_self_arg: $mut_self_ty),*)$( -> $mut_self_ret)? {
								<$state as $crate::Vision>::$mut_self_fn(unsafe { (&mut *STATE.get()).assume_init_mut() } $(, $mut_self_arg)*)
							}
						}
					)*

					$(
						$crate::paste::paste! {
							#[no_mangle]
							pub extern "Rust" fn [<$name _ $self_fn>]($($self_arg: $self_ty),*)$( -> $self_ret)? {
								<$state as $crate::Vision>::$self_fn(unsafe { (&mut *STATE.get()).assume_init_mut() } $(, $self_arg)*)
							}
						}
					)*
				}
			};
		}
	};
}
vision_dylib_wrapper! {
	Self => {};

	&mut self => {
		fn load_frame(&mut self, image: VisionFrame) -> Result<(), E>;
		// fn load_map_markers(&mut self, map_marker_size: u32) -> Result<(), E>;
	};

	&self => {
		fn thread_ctx(&self) -> Result<(), AnyError>;

		fn crop_to_map(&self, grayscale: bool) -> Result<Option<(image::RgbaImage, [u32; 4])>, E>;
		fn get_cpu_frame(&self) -> Arc<VisionFrame>;

		fn ocr_preprocess(&self) -> Result<(*const u8, usize), E>;
		fn find_scales_preprocess(&self, scales_start_y: u32) -> Result<*const SusRefCell<image::GrayImage>, E>;

		fn isolate_map_markers(&self) -> Result<(), E>;
		// fn filter_map_marker_icons(&self) -> Result<(), E>;
		fn mask_marker_lines(&self) -> Result<(), E>;
		fn find_marker_lines(&self, max_gap: u32) -> Result<SmallVec<Line<f32>, 32>, E>;
		fn find_longest_line(&self, image: &LSDImage, pt: Point<f32>, max_gap: f32) -> Result<(Line<f32>, f32), E>;

		fn get_debug_view(&self, choice: debug::DebugView) -> Option<Arc<image::RgbaImage>>;
	};
}

/// Contains types that are 1:1 transmutable with the type in the dynamic library.
///
/// They aren't actually transmuted, we just lie to the type system that this is what we get.
///
/// **These types should never be dropped**
pub mod transmuters {
	#![allow(unused)]

	#[repr(C)]
	pub struct GpuImage<T, Buf = (), Pixel = ()> {
		width: u32,
		height: u32,
		data: *mut T,
		len: usize,

		_buf: core::marker::PhantomData<Buf>,
		_pixel: core::marker::PhantomData<Pixel>
	}
	impl<T, Buf, Pixel> Drop for GpuImage<T, Buf, Pixel> {
		#[inline]
		fn drop(&mut self) {
			panic!("potential memory leak! I should be being dropped by the dylib, not the main application");
		}
	}
	unsafe impl<T, Buf, Pixel> Send for GpuImage<T, Buf, Pixel> {}
	unsafe impl<T, Buf, Pixel> Sync for GpuImage<T, Buf, Pixel> {}
}