use super::*;

#[repr(C)]
#[derive(Clone, Copy, DeviceCopy, Debug)]
pub(super) struct GPUMapMarkerPixel {
	pub r: u8,
	pub g: u8,
	pub b: u8,
	pub a: u8,
}

#[repr(C)]
#[derive(Clone, Copy, DeviceCopy, Zeroable, Debug)]
pub(super) struct GPUTemplateMatch {
	pub xy: u32,
	pub sad: u32,
}

pub(super) struct GPUMapMarkers {
	pub(super) ptrs: DeviceBuffer<DevicePointer<GPUMapMarkerPixel>>,
	pub(super) _buffers: [DeviceBuffer<GPUMapMarkerPixel>; markers::AMOUNT],
}
impl GPUMapMarkers {
	pub(super) fn new(size: u32) -> Result<Self, CudaError> {
		unsafe {
			let stream = stream!()?;

			let markers = markers::load_markers::<markers::UnfilteredMarkers>(size);

			let mut buffers = Vec::with_capacity(markers.len());
			let mut ptrs = Vec::with_capacity(markers.len());
			for marker in markers {
				let buffer = DeviceBuffer::from_slice_async(
					&marker
						.iter()
						.map(|marker_pixel| {
							let [r, g, b, a] = marker_pixel.0;
							GPUMapMarkerPixel { r, g, b, a }
						})
						.collect::<Vec<_>>(),
					&stream,
				)?;

				ptrs.push(buffer.as_device_ptr());
				buffers.push(buffer);
			}

			let markers = Self {
				_buffers: buffers.try_into().unwrap(),
				ptrs: DeviceBuffer::from_slice_async(&ptrs, &stream)?,
			};

			stream.synchronize()?;

			Ok(markers)
		}
	}

	pub(super) fn as_device_ptr(&self) -> DevicePointer<DevicePointer<GPUMapMarkerPixel>> {
		self.ptrs.as_device_ptr()
	}
}
