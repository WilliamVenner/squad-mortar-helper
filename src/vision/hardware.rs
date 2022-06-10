use smh_vision_cpu as cpu;
use smh_vision_common::{prelude::*, Vision, dylib::{self, VisionDylibWrapper}};

use super::{*, capture::Frame};

#[allow(clippy::large_enum_variant)]
pub enum VisionDelegate {
	Cpu(Option<cpu::CPUFallback>),

	#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
	Gpu(Option<gpu_support::DylibWrapper>)
}
impl VisionDelegate {
	pub fn process(&mut self, state: &mut VisionState, debug: &mut DebugBox, frame: Frame) -> Result<Option<VisionResults>, AnyError> {
		// This strange looking control flow is for allowing the user to change the hardware that is used at runtime
		#[allow(clippy::never_loop)]
		loop {
			match self {
				VisionDelegate::Cpu(cpu) => {
					#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
					if SETTINGS.hardware_acceleration() {
						drop(cpu.take());

						*self = gpu();
						continue;
					}

					return state.process(cpu.as_mut().unwrap(), frame, debug);
				},

				#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
				VisionDelegate::Gpu(gpu) => {
					if !SETTINGS.hardware_acceleration() {
						drop(gpu.take());

						*self = cpu();
						continue;
					}

					return state.process(gpu.as_mut().unwrap(), frame, debug);
				},
			}
		}
	}
}

fn cpu() -> VisionDelegate {
	#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
	SETTINGS.set_hardware_acceleration(false);

	VisionDelegate::Cpu(Some(cpu::CPUFallback::init().expect("Failed to start CPU vision")))
}

#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
pub use gpu_support::*;
#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
mod gpu_support {
	use super::*;

	pub type DylibWrapper = VisionDylibWrapper<dylib::transmuters::GpuImage<u8>, AnyError>;

	unsafe fn load_gpu() -> Result<DylibWrapper, AnyError> {
		DylibWrapper::wrap("smh_vision_gpu")
	}

	pub fn gpu() -> VisionDelegate {
		if SETTINGS.hardware_acceleration() {
			match unsafe { load_gpu() } {
				Ok(cuda) => {
					log::info!("GPU initialized!");
					VisionDelegate::Gpu(Some(cuda))
				},
				Err(err) => {
					log::error!("Error loading GPU vision: {err:?}\n\nFalling back to CPU vision...");
					cpu()
				}
			}
		} else {
			cpu()
		}
	}
}

#[cfg(not(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64")))]
pub fn gpu() -> VisionDelegate {
	log::info!("GPU support is not enabled for this build");
	cpu()
}

pub fn init() -> VisionDelegate {
	gpu() // This still respects the hardware acceleration setting
}