use super::*;

pub(crate) type CudaResult<T> = core::result::Result<T, CudaError>;

#[derive(Debug)]
pub(super) enum ThreadLocalCudaCtx {
	MainThread,
	None,
	Some
}
thread_local! {
	pub(super) static THREAD_LOCAL_CUDA_CTX: RefCell<ThreadLocalCudaCtx> = RefCell::new(ThreadLocalCudaCtx::None);
}

pub struct CUDAInstance {
	pub(super) state: super::GPUVisionState,

	module: Module,

	// context must be dropped last!
	pub(super) context: Context,
}
unsafe impl Send for CUDAInstance {}
unsafe impl Sync for CUDAInstance {}
impl CUDAInstance {
	#[inline(always)]
	pub(super) fn memory(&self) -> &super::GPUMemory {
		self.state.memory.as_ref().sus_expect("GPU memory is not initialized")
	}

	pub fn init() -> Result<CUDAInstance, AnyError> {
		cust::init(CudaFlags::empty())?;

		let device = Device::get_device(0)?;
		let context = Context::new(device)?;

		// Because this thread is the owning CUDA context, we need to mark it as so in our thread-local
		THREAD_LOCAL_CUDA_CTX.with(|cell| {
			*cell.borrow_mut() = ThreadLocalCudaCtx::MainThread;
		});

		#[allow(deprecated)] // https://github.com/Rust-GPU/Rust-CUDA/issues/70
		let module = {
			#[cfg(any(debug_assertions, feature = "force-gpu-debug"))] {
				const PTX: &str = concat!(include_str!(concat!("../cuda/cuda_dbg.ptx")), "\0");
				Module::load_from_string(unsafe { std::ffi::CStr::from_ptr(PTX.as_ptr() as *const i8) })?
			}

			#[cfg(all(not(debug_assertions), not(feature = "force-gpu-debug")))] {
				macro_rules! ptx_versions {
					($([$major:literal, $minor:literal]),*) => {
						const PTX: &'static [&'static str] = &[
							concat!(include_str!("../cuda/cuda_release.ptx"), "\0"),
							$(concat!(include_str!(concat!("../cuda/cuda_release_", stringify!($major), stringify!($minor), ".ptx")), "\0")),*
						];

						const COMPUTE_CAPABILITIES: &'static [[i32; 2]] = &[$([$major, $minor]),*];
					}
				}
				ptx_versions!(
					[8, 6],
					[7, 5],
					[6, 1],
					[5, 2],
					[3, 5]
				);

				let cc = [device.get_attribute(cust::device::DeviceAttribute::ComputeCapabilityMajor)?, device.get_attribute(cust::device::DeviceAttribute::ComputeCapabilityMinor)?];
				let device_ptx = COMPUTE_CAPABILITIES.iter().position(|compiled_cc| {
					if compiled_cc[0] > cc[0] || compiled_cc[1] > cc[1] {
						return false;
					} else {
						log::info!("GPU compute capability >= {}.{}", compiled_cc[0], compiled_cc[1]);
						return true;
					}
				});

				let device_ptx = if let Some(device_ptx) = device_ptx {
					PTX[device_ptx + 1]
				} else {
					PTX[0]
				};

				Module::load_from_string(unsafe { std::ffi::CStr::from_ptr(device_ptx.as_ptr() as *const i8) })?
			}
		};

		Ok(CUDAInstance {
			state: GPUVisionState::default(),
			context,
			module
		})
	}
}
impl core::ops::Deref for CUDAInstance {
	type Target = Module;

	#[inline]
	fn deref(&self) -> &Self::Target {
	  	&self.module
	}
}

#[macro_export]
#[allow(unused)]
macro_rules! open_gpu_image {
	(rgb $image:expr) => {{
		let image = $image.try_clone().unwrap();
		smh_vision_common::open_image!(image::RgbImage::try_from(image).unwrap());
	}};

	(luma8 $image:expr) => {{
		let image = $image.try_clone().unwrap();
		smh_vision_common::open_image!(image::GrayImage::try_from(image).unwrap());
	}};
}

macro_rules! stream {
	() => {
		Stream::new(StreamFlags::DEFAULT, i32::MIN.into())
	}
}
pub(crate) use stream;

macro_rules! gpu_2d_kernel {
	[<<<[$w:expr, $h:expr], ($blockA:expr, $blockB:expr)>>>] => {{
		let block: (u32, u32) = ($blockA, $blockB);
		let grid: (u32, u32) = (($w + block.0 - 1) / block.0, ($h + block.1 - 1) / block.1);
		(grid, block)
	}};
}
pub(crate) use gpu_2d_kernel;

#[repr(C, align(16))]
#[derive(Clone, Copy, Default, DeviceCopy, Debug, PartialEq, PartialOrd)]
pub struct CudaFloat4 {
	pub x: f32,
	pub y: f32,
	pub z: f32,
	pub w: f32
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default, DeviceCopy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CudaInt4 {
	pub x: i32,
	pub y: i32,
	pub z: i32,
	pub w: i32
}

#[repr(C, align(8))]
#[derive(Clone, Copy, Default, DeviceCopy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CudaUInt2 {
	pub x: u32,
	pub y: u32
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default, DeviceCopy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CudaUInt4 {
	pub x: u32,
	pub y: u32,
	pub z: u32,
	pub w: u32
}

#[derive(Zeroable, Clone, Copy, Debug, Default, PartialEq, Eq, DeviceCopy)]
#[repr(C)]
pub struct GPUPoint<T: Zeroable> {
	pub x: T,
	pub y: T
}

#[derive(Zeroable, Clone, Copy, Debug, Default, PartialEq, Eq, DeviceCopy)]
#[repr(C)]
pub struct GPULine<T: Zeroable> {
	pub p0: GPUPoint<T>,
	pub p1: GPUPoint<T>
}