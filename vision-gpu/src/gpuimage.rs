use super::*;

pub trait GPUBuffer<T: DeviceCopy>: Sized + Deref<Target = Self::Slice> + DerefMut {
	type Ptr: Sized;
	type Slice: ?Sized;

	unsafe fn uninitialized(len: usize) -> Result<Self, CudaError>;

	fn len(&self) -> usize;
	fn as_mut_ptr(&mut self) -> *mut T;
	unsafe fn wrap_ptr(ptr: *mut T) -> Self::Ptr;
	unsafe fn from_raw_parts(ptr: Self::Ptr, capacity: usize) -> Self;

	fn from_slice(slice: &[T]) -> cuda::CudaResult<Self>
	where
		T: Clone + Copy;

	unsafe fn from_slice_async(slice: &[T], stream: &Stream) -> cuda::CudaResult<Self>
	where
		T: Clone + Copy;
}
impl<T: GPUImagePrimitive> GPUBuffer<T> for DeviceBuffer<T> {
	type Ptr = DevicePointer<T>;
	type Slice = DeviceSlice<T>;

	#[inline(always)]
	unsafe fn uninitialized(len: usize) -> Result<Self, CudaError> {
		DeviceBuffer::uninitialized(len)
	}

	#[inline(always)]
	unsafe fn from_raw_parts(ptr: Self::Ptr, capacity: usize) -> Self {
		DeviceBuffer::<T>::from_raw_parts(ptr, capacity)
	}

	#[inline(always)]
	unsafe fn wrap_ptr(ptr: *mut T) -> Self::Ptr {
		DevicePointer::from_raw(ptr as usize as u64)
	}

	#[inline(always)]
	fn as_mut_ptr(&mut self) -> *mut T {
		self.deref_mut().as_device_ptr().as_mut_ptr()
	}

	#[inline(always)]
	fn len(&self) -> usize {
		self.deref().len()
	}

	#[inline(always)]
	fn from_slice(slice: &[T]) -> cuda::CudaResult<Self>
	where
		T: Clone + Copy,
	{
		DeviceBuffer::from_slice(slice)
	}

	#[inline(always)]
	unsafe fn from_slice_async(slice: &[T], stream: &Stream) -> cuda::CudaResult<Self>
	where
		T: Clone + Copy,
	{
		DeviceBuffer::from_slice_async(slice, stream)
	}
}
impl<T: GPUImagePrimitive> GPUBuffer<T> for UnifiedBuffer<T> {
	type Ptr = UnifiedPointer<T>;
	type Slice = [T];

	#[inline(always)]
	unsafe fn uninitialized(len: usize) -> Result<Self, CudaError> {
		UnifiedBuffer::uninitialized(len)
	}

	#[inline(always)]
	unsafe fn from_raw_parts(ptr: Self::Ptr, capacity: usize) -> Self {
		UnifiedBuffer::<T>::from_raw_parts(ptr, capacity)
	}

	#[inline(always)]
	unsafe fn wrap_ptr(ptr: *mut T) -> Self::Ptr {
		UnifiedPointer::wrap(ptr)
	}

	#[inline(always)]
	fn as_mut_ptr(&mut self) -> *mut T {
		self.deref_mut().as_mut_ptr()
	}

	#[inline(always)]
	fn len(&self) -> usize {
		self.deref().len()
	}

	#[inline(always)]
	fn from_slice(slice: &[T]) -> cuda::CudaResult<Self>
	where
		T: Clone + Copy,
	{
		UnifiedBuffer::from_slice(slice)
	}

	#[inline(always)]
	unsafe fn from_slice_async(slice: &[T], _stream: &Stream) -> cuda::CudaResult<Self>
	where
		T: Clone + Copy,
	{
		Self::from_slice(slice)
	}
}

pub trait GPUImagePrimitive: Send + Sync + DeviceCopy + image::Primitive + 'static {}
impl<T: Send + Sync + DeviceCopy + image::Primitive + 'static> GPUImagePrimitive for T {}

/// The pinned variant provides fast copying from the GPU to the host
pub struct PinnedGPUImage<T = u8, Buf = DeviceBuffer<T>, Pixel = image::Rgb<T>>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	inner: GPUImage<T, Buf, Pixel>,
	locked_buffer: LockedBuffer<T>
}
impl<T, Buf, Pixel> PinnedGPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	#[inline]
	pub unsafe fn uninitialized(w: u32, h: u32, channels: usize) -> Result<Self, CudaError> {
		Ok(Self {
			inner: GPUImage::uninitialized(w, h, channels)?,
			locked_buffer: {
				let mut buffer = LockedBuffer::uninitialized(w as usize * h as usize * channels)?;
				core::ptr::write_bytes(buffer.as_mut_ptr(), 0, buffer.len());
				buffer
			}
		})
	}

	#[inline]
	pub fn async_copy_from_gpu(&mut self, stream: &Stream) -> Result<image::ImageBuffer<Pixel, &[T]>, CudaError>
	where
		Buf::Slice: AsyncCopyDestination<LockedBuffer<T>>
	{
		self.inner.async_copy_to(&mut self.locked_buffer, stream)?;
		Ok(self.as_host_ref())
	}

	#[inline]
	#[allow(unused)]
	pub fn copy_from_gpu(&mut self) -> Result<image::ImageBuffer<Pixel, &[T]>, CudaError>
	where
		Buf::Slice: CopyDestination<[T]>
	{
		self.inner.copy_to(&mut self.locked_buffer)?;
		Ok(self.as_host_ref())
	}

	#[inline]
	pub fn as_host_ref(&self) -> image::ImageBuffer<Pixel, &[T]> {
		image::ImageBuffer::from_raw(self.inner.width, self.inner.height, &*self.locked_buffer).sus_unwrap()
	}
}
impl<T, Buf, Pixel> core::ops::Deref for PinnedGPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	type Target = GPUImage<T, Buf, Pixel>;

	#[inline(always)]
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}
impl<T, Buf, Pixel> core::ops::DerefMut for PinnedGPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	#[inline(always)]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

#[repr(C)]
pub struct GPUImage<T = u8, Buf = DeviceBuffer<T>, Pixel = image::Rgb<T>>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	pub(crate) width: u32,
	pub(crate) height: u32,

	// this needs to be a raw pointer so we can pass it along the dylib boundary
	pub(crate) data: *mut T,
	pub(crate) len: usize,

	_buf: PhantomData<Buf>,
	_pixel: PhantomData<Pixel>,
}

unsafe impl<T, Buf, Pixel> Send for GPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
}

unsafe impl<T, Buf, Pixel> Sync for GPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
}

impl<T, Pixel> GPUImage<T, DeviceBuffer<T>, Pixel>
where
	T: GPUImagePrimitive,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	#[inline(always)]
	pub fn as_device_ptr(&self) -> DevicePointer<T> {
		DevicePointer::from_raw(self.data as usize as u64)
	}
}

impl<T, Pixel> GPUImage<T, UnifiedBuffer<T>, Pixel>
where
	T: GPUImagePrimitive,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	#[inline(always)]
	pub fn as_unified_ptr(&self) -> UnifiedPointer<T> {
		unsafe { UnifiedPointer::wrap(self.data) }
	}

	#[inline(always)]
	pub fn as_slice(&self) -> &[T] {
		unsafe { core::slice::from_raw_parts(self.data, self.len) }
	}
}

impl<T, Buf, Pixel> Drop for GPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	#[inline]
	fn drop(&mut self) {
		// free the CUDA memory
		drop(unsafe { self.conjure_cuda_buffer() });
		self.data = core::ptr::null_mut();
	}
}
impl<T, Buf, Pixel> GPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	#[inline]
	pub fn new(w: u32, h: u32, data: Buf) -> Self {
		Self {
			width: w,
			height: h,
			len: data.len(),
			data: unsafe { Self::conjure_data_ptr(data) },
			_buf: Default::default(),
			_pixel: Default::default(),
		}
	}

	#[inline]
	pub unsafe fn uninitialized(w: u32, h: u32, channels: usize) -> Result<Self, CudaError> {
		Ok(Self {
			data: Self::conjure_data_ptr(Buf::uninitialized(w as usize * h as usize * channels)?),
			len: w as usize * h as usize * channels,
			width: w,
			height: h,
			_buf: Default::default(),
			_pixel: Default::default(),
		})
	}

	#[inline(always)]
	pub fn len(&self) -> usize {
		self.len
	}

	#[must_use]
	#[inline(always)]
	unsafe fn conjure_data_ptr(mut device_buffer: Buf) -> *mut T {
		let ptr = device_buffer.as_mut_ptr();
		std::mem::forget(device_buffer);
		ptr
	}

	#[must_use]
	#[inline(always)]
	/// # **THIS WILL FREE CUDA MEMORY ON DROP**
	unsafe fn conjure_cuda_buffer(&self) -> Buf {
		Buf::from_raw_parts(Buf::wrap_ptr(self.data), self.len())
	}

	#[inline(always)]
	unsafe fn conjure_temp_cuda_buffer<R, F: FnOnce(&mut Buf) -> R>(&self, f: F) -> R {
		let mut buf = Buf::from_raw_parts(Buf::wrap_ptr(self.data), self.len());
		let ret = f(&mut buf);
		std::mem::forget(buf);
		ret
	}

	#[inline]
	#[allow(unused)]
	pub unsafe fn async_try_from<C: core::ops::Deref<Target = [Pixel::Subpixel]>>(image: &image::ImageBuffer<Pixel, C>, stream: &Stream) -> cust::error::CudaResult<Self> {
		Ok(Self {
			width: image.width(),
			height: image.height(),
			len: image.len(),
			data: unsafe { Self::conjure_data_ptr(Buf::from_slice_async(image.as_raw(), stream)?) },
			_buf: Default::default(),
			_pixel: Default::default(),
		})
	}

	#[inline]
	#[allow(unused)]
	pub fn copy_from<O>(&mut self, source: &O) -> cust::error::CudaResult<()>
	where
		O: ?Sized + AsRef<[T]> + AsMut<[T]>,
		Buf::Slice: CopyDestination<O>,
	{
		unsafe {
			self.conjure_temp_cuda_buffer(|buffer| CopyDestination::copy_from(buffer.deref_mut(), source))
		}
	}

	#[inline]
	#[allow(unused)]
	pub fn copy_to<O>(&self, dest: &mut O) -> cust::error::CudaResult<()>
	where
		O: ?Sized + AsRef<[T]> + AsMut<[T]>,
		Buf::Slice: CopyDestination<O>,
	{
		unsafe { self.conjure_temp_cuda_buffer(|buffer| CopyDestination::copy_to(buffer.deref_mut(), dest)) }
	}

	#[inline]
	#[allow(unused)]
	pub fn async_copy_from<O>(&mut self, source: &O, stream: &Stream) -> cust::error::CudaResult<()>
	where
		O: ?Sized + AsRef<[T]> + AsMut<[T]>,
		Buf::Slice: AsyncCopyDestination<O>,
	{
		unsafe {
			self.conjure_temp_cuda_buffer(|buffer| {
				AsyncCopyDestination::async_copy_from(buffer.deref_mut(), source, stream)
			})
		}
	}

	#[inline]
	#[allow(unused)]
	pub fn async_copy_to<O>(&self, dest: &mut O, stream: &Stream) -> cust::error::CudaResult<()>
	where
		O: ?Sized + AsRef<[T]> + AsMut<[T]>,
		Buf::Slice: AsyncCopyDestination<O>,
	{
		unsafe {
			self.conjure_temp_cuda_buffer(|buffer| {
				AsyncCopyDestination::async_copy_to(buffer.deref_mut(), dest, stream)
			})
		}
	}
}

impl<T, Buf, Pixel> GPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive + Default + Clone + Copy,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
	Buf::Slice: CopyDestination<[T]>,
{
	pub fn try_clone(&self) -> Result<Self, CudaError> {
		Ok(Self {
			width: self.width,
			height: self.height,
			len: self.len,
			data: {
				let mut buffer = vec![T::default(); self.len()];
				self.copy_to(&mut buffer)?;
				unsafe { Self::conjure_data_ptr(Buf::from_slice(&buffer)?) }
			},
			_buf: Default::default(),
			_pixel: Default::default(),
		})
	}
}
impl<T, Buf, Pixel> GPUImage<T, Buf, Pixel>
where
	T: GPUImagePrimitive + Default + Clone + Copy,
	Buf: GPUBuffer<T>,
	Pixel: image::Pixel<Subpixel = T> + 'static,
{
	pub fn from_raw(w: u32, h: u32, buf: Vec<T>) -> Result<Option<Self>, CudaError> {
		if buf.len() == w as usize * h as usize {
			Ok(Some(Self {
				width: w,
				height: h,
				len: buf.len(),
				data: unsafe { Self::conjure_data_ptr(Buf::from_slice(&buf)?) },
				_buf: Default::default(),
				_pixel: Default::default(),
			}))
		} else {
			Ok(None)
		}
	}
}
impl<P, Buf> TryFrom<&image::ImageBuffer<P, Vec<u8>>> for GPUImage<u8, Buf>
where
	P: image::Pixel<Subpixel = u8> + 'static,
	P::Subpixel: 'static,
	Vec<u8>: core::ops::Deref<Target = [P::Subpixel]>,
	Buf: GPUBuffer<u8>,
{
	type Error = CudaError;

	fn try_from(image: &image::ImageBuffer<P, Vec<u8>>) -> Result<Self, Self::Error> {
		Ok(Self {
			width: image.width(),
			height: image.height(),
			len: image.len(),
			data: unsafe { Self::conjure_data_ptr(Buf::from_slice(image.as_raw())?) },
			_buf: Default::default(),
			_pixel: Default::default(),
		})
	}
}
impl<P, Buf> TryFrom<image::ImageBuffer<P, Vec<u8>>> for GPUImage<u8, Buf>
where
	P: image::Pixel<Subpixel = u8> + 'static,
	P::Subpixel: 'static,
	Vec<u8>: core::ops::Deref<Target = [P::Subpixel]>,
	Buf: GPUBuffer<u8>,
{
	type Error = CudaError;

	fn try_from(image: image::ImageBuffer<P, Vec<u8>>) -> Result<Self, Self::Error> {
		GPUImage::try_from(&image)
	}
}
impl<'a, P, Buf> TryFrom<&image::ImageBuffer<P, &'a [u8]>> for GPUImage<u8, Buf>
where
	P: image::Pixel<Subpixel = u8> + 'static,
	P::Subpixel: 'static,
	&'a [u8]: core::ops::Deref<Target = [P::Subpixel]>,
	Buf: GPUBuffer<u8>,
{
	type Error = CudaError;

	fn try_from(image: &image::ImageBuffer<P, &'a [u8]>) -> Result<Self, Self::Error> {
		Ok(Self {
			width: image.width(),
			height: image.height(),
			len: image.len(),
			data: unsafe { Self::conjure_data_ptr(Buf::from_slice(image.as_raw())?) },
			_buf: Default::default(),
			_pixel: Default::default(),
		})
	}
}
impl<'a, P> TryFrom<image::ImageBuffer<P, &'a [u8]>> for GPUImage<u8>
where
	P: image::Pixel<Subpixel = u8> + 'static,
	P::Subpixel: 'static,
	&'a [u8]: core::ops::Deref<Target = [P::Subpixel]>,
{
	type Error = CudaError;

	fn try_from(image: image::ImageBuffer<P, &'a [u8]>) -> Result<Self, Self::Error> {
		GPUImage::try_from(&image)
	}
}
impl TryFrom<&GPUImage<u8>> for image::RgbImage {
	type Error = CudaError;

	fn try_from(image: &GPUImage<u8>) -> Result<Self, Self::Error> {
		let mut buffer = vec![0u8; image.width as usize * image.height as usize * 3];
		image.copy_to(&mut buffer)?;
		Self::from_raw(image.width, image.height, buffer).ok_or(CudaError::InvalidMemoryAllocation)
	}
}
impl TryFrom<&GPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>> for image::GrayImage {
	type Error = CudaError;

	fn try_from(image: &GPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>) -> Result<Self, Self::Error> {
		let mut buffer = vec![0u8; image.width as usize * image.height as usize];
		image.copy_to(&mut buffer)?;
		Self::from_raw(image.width, image.height, buffer).ok_or(CudaError::InvalidMemoryAllocation)
	}
}
impl TryFrom<GPUImage<u8>> for image::RgbImage {
	type Error = CudaError;

	fn try_from(image: GPUImage<u8>) -> Result<Self, Self::Error> {
		image::RgbImage::try_from(&image)
	}
}
impl TryFrom<GPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>> for image::GrayImage {
	type Error = CudaError;

	fn try_from(image: GPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>) -> Result<Self, Self::Error> {
		image::GrayImage::try_from(&image)
	}
}

impl image::GenericImageView for GPUImage<u8, UnifiedBuffer<u8>, image::Luma<u8>> {
	type Pixel = image::Luma<u8>;
	type InnerImageView = Self;

	#[inline]
	fn dimensions(&self) -> (u32, u32) {
		(self.width, self.height)
	}

	#[inline]
	fn bounds(&self) -> (u32, u32, u32, u32) {
		(0, 0, self.width, self.height)
	}

	#[inline]
	fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
		if x < self.width && y < self.height {
			unsafe { self.unsafe_get_pixel(x, y) }
		} else {
			panic!("Image index {:?} out of bounds {:?}", (x, y), (self.width, self.height))
		}
	}

	#[inline]
	unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
		image::Luma([self.conjure_temp_cuda_buffer(|buf| *buf.get_unchecked(y as usize * self.width as usize + x as usize))])
	}

	#[inline]
	fn inner(&self) -> &Self::InnerImageView {
		self
	}
}
impl image::GenericImage for GPUImage<u8, UnifiedBuffer<u8>, image::Luma<u8>> {
	type InnerImage = Self;

	#[inline(always)]
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
		if x < self.width && y < self.height {
			let idx = y as usize * self.width as usize + x as usize;
			assert!(idx < self.len);
			<Self::Pixel as image::Pixel>::from_slice_mut(core::slice::from_mut(unsafe {
				core::slice::from_raw_parts_mut(self.data, self.len).get_unchecked_mut(idx)
			}))
		} else {
			panic!("Image index {:?} out of bounds {:?}", (x, y), (self.width, self.height))
		}
	}

	#[inline(always)]
	fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		*self.get_pixel_mut(x, y) = pixel;
	}

	#[inline(always)]
	unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		let idx = y as usize * self.width as usize + x as usize;
		assert!(idx < self.len);
		self.conjure_temp_cuda_buffer(|buffer| *buffer.get_unchecked_mut(idx) = pixel.0[0]);
	}

	#[inline(always)]
	fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		image::Pixel::blend(self.get_pixel_mut(x, y), &pixel)
	}

	#[inline(always)]
	fn inner_mut(&mut self) -> &mut Self::InnerImage {
		self
	}
}

impl image::GenericImageView for GPUImage<u8, UnifiedBuffer<u8>, image::Rgb<u8>> {
	type Pixel = image::Rgb<u8>;
	type InnerImageView = Self;

	#[inline]
	fn dimensions(&self) -> (u32, u32) {
		(self.width, self.height)
	}

	#[inline]
	fn bounds(&self) -> (u32, u32, u32, u32) {
		(0, 0, self.width, self.height)
	}

	#[inline]
	fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
		if x < self.width && y < self.height {
			unsafe { self.unsafe_get_pixel(x, y) }
		} else {
			panic!("Image index {:?} out of bounds {:?}", (x, y), (self.width, self.height))
		}
	}

	#[inline(always)]
	unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
		let mut pixel = [0u8; <Self::Pixel as image::Pixel>::CHANNEL_COUNT as usize];
		self.conjure_temp_cuda_buffer(|buf| {
			pixel.copy_from_slice(buf.get_unchecked({
				let idx = (y as usize * self.width as usize + x as usize) * <Self::Pixel as image::Pixel>::CHANNEL_COUNT as usize;
				idx..idx + <Self::Pixel as image::Pixel>::CHANNEL_COUNT as usize
			}));
		});
		image::Rgb(pixel)
	}

	#[inline(always)]
	fn inner(&self) -> &Self::InnerImageView {
		self
	}
}
impl image::GenericImage for GPUImage<u8, UnifiedBuffer<u8>, image::Rgb<u8>> {
	type InnerImage = Self;

	#[inline(always)]
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
		if x < self.width && y < self.height {
			<Self::Pixel as image::Pixel>::from_slice_mut(unsafe {
				core::slice::from_raw_parts_mut(self.data, self.len).get_unchecked_mut({
					let idx = (y as usize * self.width as usize + x as usize) * <Self::Pixel as image::Pixel>::CHANNEL_COUNT as usize;
					idx..idx + <Self::Pixel as image::Pixel>::CHANNEL_COUNT as usize
				})
			})
		} else {
			panic!("Image index {:?} out of bounds {:?}", (x, y), (self.width, self.height))
		}
	}

	#[inline(always)]
	fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		*self.get_pixel_mut(x, y) = pixel;
	}

	#[inline(always)]
	unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		self.conjure_temp_cuda_buffer(|buf| {
			buf.get_unchecked_mut({
				let idx = (y as usize * self.width as usize + x as usize) * <Self::Pixel as image::Pixel>::CHANNEL_COUNT as usize;
				idx..idx + <Self::Pixel as image::Pixel>::CHANNEL_COUNT as usize
			})
			.copy_from_slice(&pixel.0);
		})
	}

	#[inline(always)]
	fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		image::Pixel::blend(self.get_pixel_mut(x, y), &pixel)
	}

	#[inline(always)]
	fn inner_mut(&mut self) -> &mut Self::InnerImage {
		self
	}
}