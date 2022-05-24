// TODO make block sizes configurable ("GPU load" slider)
// TODO explore performance impact of cudaStreamAttachMemAsync

#[macro_use]
extern crate bytemuck;

mod cuda;
mod gpuimage;
mod gpumarkers;

smh_vision_common::export_dylib_wrapper!(smh_vision_gpu => cuda::CUDAInstance);

#[allow(unused)]
use smh_vision_common::open_image;
use smh_vision_common::{consts::*, prelude::*, Vision};

use bytemuck::Zeroable;
use cust::{
	context::CurrentContext,
	error::CudaError,
	launch,
	memory::{
		AsyncCopyDestination, CopyDestination, DeviceBox, DeviceBuffer, DeviceCopy, DevicePointer, DeviceSlice, LockedBuffer, UnifiedBuffer,
		UnifiedPointer, DeviceSliceIndex,
	},
	prelude::*,
};

use cuda::*;
use gpuimage::*;
use gpumarkers::*;

struct GPUMemory {
	frame: GPUImage<u8, DeviceBuffer<u8>, image::Bgra<u8>>,

	red_pixels: DeviceBox<u32>,
	ui_map: GPUImage<u8, DeviceBuffer<u8>, image::Rgba<u8>>,
	cropped_map: GPUImage<u8, DeviceBuffer<u8>, image::Rgb<u8>>,
	cropped_brq: GPUImage<u8, DeviceBuffer<u8>, image::Rgb<u8>>,

	ocr_out: SusRefCell<PinnedGPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>>,

	scales_preprocessed: GPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>,
	scales_preprocessed_host: SusRefCell<image::GrayImage>,

	map_marker_template_matches_sad: SusRefCell<Vec<GPUTemplateMatch>>,
	marked_map_marker_pixels: DeviceBuffer<GPUTemplateMatch>,
	marked_map_marker_pixels_count: DeviceBox<u32>,

	lsd_image: SusRefCell<PinnedGPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>>,
	longest_lines: DeviceBuffer<GPULine<f32>>,

	crop_to_map_streams: (Stream, Stream),
	markers_stream: Stream,
	scales_stream: Stream,
}
impl GPUMemory {
	fn new(dimensions: (u32, u32)) -> Result<Self, AnyError> {
		unsafe {
			let map_icon_size: u32 = 22; // TODO scale to monitor

			let [_, _, w, h] = MAP_BOUNDS.into_absolute([dimensions.0, dimensions.1]);

			// Map fills remaining space
			let w = dimensions.0 - w;

			// brq = bottom right quadrant
			let brq_w = w / 2;
			let brq_h = h / 2;

			// TODO we can do all of this async
			// TODO this should all be done in the kernel-calling function

			Ok(Self {
				crop_to_map_streams: (stream!()?, stream!()?),
				markers_stream: stream!()?,
				scales_stream: stream!()?,

				frame: GPUImage::uninitialized(dimensions.0, dimensions.1, 4)?,

				red_pixels: DeviceBox::uninitialized()?,
				ui_map: GPUImage::uninitialized(w, h, 4)?,
				cropped_map: GPUImage::uninitialized(w, h, 3)?,
				cropped_brq: GPUImage::uninitialized(brq_w, brq_h, 3)?,

				ocr_out: PinnedGPUImage::uninitialized(brq_w, brq_h, 1)?.into(),

				scales_preprocessed: GPUImage::uninitialized(brq_w, brq_h, 1)?,
				scales_preprocessed_host: SusRefCell::new(image::GrayImage::new(brq_w, brq_h)),

				map_marker_template_matches_sad: SusRefCell::new(vec![GPUTemplateMatch::zeroed(); (w - map_icon_size) as usize * (h - map_icon_size) as usize]),
				marked_map_marker_pixels: DeviceBuffer::uninitialized(w as usize * h as usize)?,
				marked_map_marker_pixels_count: DeviceBox::uninitialized()?,

				lsd_image: PinnedGPUImage::uninitialized(w, h, 1)?.into(),
				longest_lines: DeviceBuffer::uninitialized(8)?,
			})
		}
	}
}

#[derive(Default)]
struct GPUVisionState {
	cpu_frame: Arc<VisionFrame>,

	dimensions: (u32, u32),
	memory: Option<GPUMemory>,

	map_marker_size: u32,
	map_markers: Option<GPUMapMarkers>
}
impl GPUVisionState {
	#[inline]
	fn update(&mut self, dimensions: (u32, u32)) -> Result<&mut GPUMemory, AnyError> {
		if self.memory.is_none() || self.dimensions != dimensions {
			self.memory = None;
			self.memory = Some(GPUMemory::new(dimensions)?);
			self.dimensions = dimensions;
		}
		Ok(unsafe { self.memory.as_mut().unwrap_unchecked() })
	}

	#[inline]
	fn update_map_markers(&mut self, map_marker_size: u32) -> Result<(), AnyError> {
		if self.map_markers.is_none() || self.map_marker_size != map_marker_size {
			self.map_markers = None;
			self.map_markers = Some(GPUMapMarkers::new(map_marker_size)?);
			self.map_marker_size = map_marker_size;
		}
		Ok(())
	}
}

macro_rules! memory {
	(&$self:ident.$item:ident) => {
		&$self.memory().$item
	};
}

impl Vision for CUDAInstance {
	type LSDImage = GPUImage<u8, DeviceBuffer<u8>, image::Luma<u8>>;
	type Error = AnyError;

	fn init() -> Result<Self, AnyError> {
		CUDAInstance::init()
	}

	fn thread_ctx(&self) -> Result<(), smh_vision_common::prelude::AnyError> {
		cuda::THREAD_LOCAL_CUDA_CTX.with(|ctx| {
			let mut cell = ctx.borrow_mut();

			if matches!(&*cell, cuda::ThreadLocalCudaCtx::None) {
				CurrentContext::set_current(&self.context)?;
				*cell = cuda::ThreadLocalCudaCtx::Some;
			}

			Ok(())
		})
	}

	fn load_frame(&mut self, frame: VisionFrame) -> Result<(), Self::Error> {
		// preallocate buffers
		let memory = self.state.update(frame.dimensions())?;

		unsafe {
			let stream = &memory.crop_to_map_streams.0;

			// upload frame to GPU
			if frame.inner().bounds() != frame.bounds() {
				memory.frame = GPUImage::async_try_from(&frame.to_image(), stream)?;
			} else {
				memory.frame = GPUImage::async_try_from(frame.inner(), stream)?;
			}

			// reset state
			memory.red_pixels.async_copy_from(&0, stream)?;

			memory.marked_map_marker_pixels_count.async_copy_from(&0, stream)?;

			memory.map_marker_template_matches_sad.borrow_mut().fill(GPUTemplateMatch::zeroed());

			stream.synchronize()?;
		}

		self.state.cpu_frame = Arc::new(frame);

		Ok(())
	}

	#[inline]
	fn get_cpu_frame(&self) -> Arc<VisionFrame> {
		self.state.cpu_frame.clone()
	}

	#[inline]
	fn load_map_markers(&mut self, map_marker_size: u32) -> Result<(), Self::Error> {
		self.state.update_map_markers(map_marker_size)
	}

	fn crop_to_map(&self) -> Result<Option<(image::RgbaImage, [u32; 4])>, Self::Error> {
		let (stream_a, stream_b) = memory!(&self.crop_to_map_streams);

		let frame = memory!(&self.frame);

		let ui_map = memory!(&self.ui_map);
		let cropped_map = memory!(&self.cropped_map);
		let cropped_brq = memory!(&self.cropped_brq);

		let device_red_pixels = memory!(&self.red_pixels);
		let mut red_pixels = 0;

		unsafe {
			// Find the red "Close Deployment" button
			// TODO: support the quick map as well
			let [x, y, w, h] = CLOSE_DEPLOYMENT_BUTTON_BOUNDS.into_absolute([frame.width, frame.height]);

			let (grid, block) = gpu_2d_kernel![<<<[w, h], (16, 16)>>>];
			launch!(
				self.count_close_deployment_button_red_pixels<<<grid, block, 4, stream_a>>>(
					frame.as_device_ptr(),
					frame.width,
					x, y, w, h,
					device_red_pixels.as_device_ptr()
				)
			)?;

			device_red_pixels.async_copy_to(&mut red_pixels, stream_a)?;

			stream_a.synchronize()?;

			let red_pixels = red_pixels as f32 / (w * h) as f32;
			if red_pixels < CLOSE_DEPLOYMENT_BUTTON_RED_PIXEL_THRESHOLD {
				return Ok(None);
			}
		}

		let [x, y, w, h] = MAP_BOUNDS.into_absolute([frame.width, frame.height]);

		// Map fills remaining space
		let w = frame.width - w;
		let x = frame.width - x - w;

		// brq = bottom right quadrant
		let brq_w = w / 2;
		let brq_h = h / 2;

		unsafe {
			let (grid, block) = gpu_2d_kernel![<<<[w, h], (8, 8)>>>];
			launch!(
				self.crop_to_map<<<grid, block, 0, stream_a>>>(
					frame.as_device_ptr(), // BGRA
					frame.width,
					x, y, w, h,
					cropped_map.as_device_ptr(), // RGB
					ui_map.as_device_ptr() // RGBA
				)
			)?;

			let (grid, block) = gpu_2d_kernel![<<<[brq_w, brq_h], (8, 8)>>>];
			launch!(
				self.crop_to_map_brq<<<grid, block, 0, stream_b>>>(
					frame.as_device_ptr(), // BGRA
					frame.width,
					x + brq_w, y + brq_h, brq_w, brq_h,
					cropped_brq.as_device_ptr() // RGB
				)
			)?;
		}

		let mut ui_map_host = vec![0u8; w as usize * h as usize * 4];
		ui_map.async_copy_to(&mut ui_map_host, stream_a)?;

		stream_a.synchronize()?;
		stream_b.synchronize()?;

		Ok(Some((image::RgbaImage::from_vec(w, h, ui_map_host).sus_unwrap(), [x, y, w, h])))
	}

	fn ocr_preprocess(&self) -> Result<(*const u8, usize), Self::Error> {
		let stream = memory!(&self.scales_stream);
		let cropped_brq = memory!(&self.cropped_brq);
		let mut ocr_out = memory!(&self.ocr_out).borrow_mut();

		unsafe {
			let (grid, block) = gpu_2d_kernel![<<<[cropped_brq.width, cropped_brq.height], (8, 8)>>>];
			launch!(
				self.ocr_preprocess<<<grid, block, 0, stream>>>(
					cropped_brq.as_device_ptr(),
					cropped_brq.width, cropped_brq.height,
					ocr_out.as_device_ptr()
				)
			)?;
		}

		let ocr_out = ocr_out.async_copy_from_gpu(stream)?;

		stream.synchronize()?;

		Ok((ocr_out.as_ptr(), ocr_out.len()))
	}

	fn find_scales_preprocess(&self, scales_start_y: u32) -> Result<*const SusRefCell<image::GrayImage>, Self::Error> {
		let stream = memory!(&self.scales_stream);
		let cropped_brq = memory!(&self.cropped_brq);
		let scales_preprocessed = memory!(&self.scales_preprocessed);
		let mut scales_preprocessed_host = memory!(&self.scales_preprocessed_host).borrow_mut();

		unsafe {
			let (grid, block) = gpu_2d_kernel![<<<[cropped_brq.width, cropped_brq.height], (8, 8)>>>];
			launch!(
				self.find_scales_preprocess<<<grid, block, 0, stream>>>(
					cropped_brq.as_device_ptr(),
					cropped_brq.width, cropped_brq.height,
					scales_start_y,
					scales_preprocessed.as_device_ptr()
				)
			)?;
		}

		scales_preprocessed.async_copy_to(scales_preprocessed_host.as_mut(), stream)?;

		stream.synchronize()?;

		Ok(memory!(&self.scales_preprocessed_host) as *const _)
	}

	fn isolate_map_markers(&self) -> Result<(), Self::Error> {
		let stream = memory!(&self.markers_stream);
		let cropped_map = memory!(&self.cropped_map);
		let marked_map_marker_pixels = memory!(&self.marked_map_marker_pixels);
		let marked_map_marker_pixels_count = memory!(&self.marked_map_marker_pixels_count);
		let map_marker_size = self.state.map_marker_size;

		unsafe {
			let (grid, block) = gpu_2d_kernel![<<<[cropped_map.width, cropped_map.height], (8, 8)>>>];
			launch!(
				self.isolate_map_markers<<<grid, block, 0, stream>>>(
					cropped_map.as_device_ptr(),
					cropped_map.width, cropped_map.height,
					marked_map_marker_pixels.as_device_ptr(),
					marked_map_marker_pixels_count.as_device_ptr(),
					map_marker_size
				)
			)?;
		}

		stream.synchronize()?;

		Ok(())
	}

	fn filter_map_marker_icons(&self) -> Result<(), Self::Error> {
		// TODO verify and clean up this code i wrote when very tired

		let marked_map_marker_pixels_count = memory!(&self.marked_map_marker_pixels_count).as_host_value()?;
		if marked_map_marker_pixels_count == 0 {
			return Ok(());
		}

		let stream = memory!(&self.markers_stream);
		let cropped_map = memory!(&self.cropped_map);

		let map_marker_size = self.state.map_marker_size;
		let map_markers = self.state.map_markers.as_ref().sus_unwrap();

		let mut map_marker_template_matches_sad = memory!(&self.map_marker_template_matches_sad).borrow_mut();
		let marked_map_marker_pixels = memory!(&self.marked_map_marker_pixels);

		unsafe {
			let (grid, block) = gpu_2d_kernel![<<<[markers::AMOUNT as u32, marked_map_marker_pixels_count], (8, 8)>>>];
			launch!(
				self.filter_map_marker_icons<<<grid, block, 0, stream>>>(
					cropped_map.as_device_ptr(),
					cropped_map.width,

					marked_map_marker_pixels.as_device_ptr(),

					map_markers.as_device_ptr(),
					map_marker_size,

					markers::AMOUNT as u32,
					marked_map_marker_pixels_count
				)
			)?;

			stream.synchronize()?;

			let map_marker_template_matches_sad = &mut map_marker_template_matches_sad[0..marked_map_marker_pixels_count as usize];
			(0..marked_map_marker_pixels_count as usize).index(marked_map_marker_pixels.as_slice()).copy_to(map_marker_template_matches_sad)?;

			let min_sad_xy = map_marker_template_matches_sad.par_iter().copied().min_by_key(|template_match| template_match.sad).map(|template_match| template_match.xy);
			if let Some(min_sad_xy) = min_sad_xy {
				let (grid, block) = gpu_2d_kernel![<<<[map_marker_size, map_marker_size], (8, 8)>>>];
				launch!(
					self.filter_map_marker_icons_clear<<<grid, block, 0, stream>>>(
						cropped_map.as_device_ptr(),
						cropped_map.width, cropped_map.height,

						min_sad_xy,
						map_marker_size
					)
				)?;

				stream.synchronize()?;
			}
		}

		Ok(())
	}

	fn mask_marker_lines(&self) -> Result<(), Self::Error> {
		let stream = memory!(&self.markers_stream);

		let cropped_map = memory!(&self.cropped_map);
		let mut lsd_image = memory!(&self.lsd_image).borrow_mut();

		unsafe {
			let (grid, block) = gpu_2d_kernel![<<<[cropped_map.width, cropped_map.height], (8, 8)>>>];
			launch!(
				self.mask_marker_lines<<<grid, block, 0, stream>>>(
					cropped_map.as_device_ptr(),
					cropped_map.width, cropped_map.height,
					lsd_image.as_device_ptr()
				)
			)?;
		}

		lsd_image.async_copy_from_gpu(stream)?;

		stream.synchronize()?;

		Ok(())
	}

	fn find_marker_lines(&self, max_gap: u32) -> Result<SmallVec<Line<f32>, 32>, Self::Error> {
		let lsd_image = memory!(&self.lsd_image).borrow();
		let lsd_image_host = lsd_image.as_host_ref();

		lsd::find_lines(
			&lsd_image_host,
			max_gap,
			#[inline]
			|_, pt, max_gap| self.find_longest_line(&*lsd_image, pt, max_gap),
		)
	}

	fn find_longest_line(&self, image: &Self::LSDImage, pt: Point<f32>, max_gap: f32) -> Result<(Line<f32>, f32), Self::Error> {
		let stream = memory!(&self.markers_stream);

		let longest_lines = memory!(&self.longest_lines);

		unsafe {
			launch!(
				self.find_longest_line<<<8, 7200 / 8, 0, stream>>>(
					image.as_device_ptr(),
					image.width, image.height,

					GPUPoint { x: pt.x, y: pt.y },
					max_gap,

					longest_lines.as_device_ptr()
				)
			)?;

			let mut block_longest_lines = [GPULine::zeroed(); 8];
			longest_lines.async_copy_to(&mut block_longest_lines, stream)?;

			stream.synchronize()?;

			let (longest_line, longest_line_length) =
				block_longest_lines
					.into_iter()
					.fold((GPULine::zeroed(), 0.0), |(longest_line, longest_line_length), line| {
						let line_length = (line.p0.x - line.p1.x).powi(2) + (line.p0.y - line.p1.y).powi(2);
						if line_length > longest_line_length {
							(line, line_length)
						} else {
							(longest_line, longest_line_length)
						}
					});

			Ok((
				Line {
					p0: Point {
						x: longest_line.p0.x,
						y: longest_line.p0.y,
					},
					p1: Point {
						x: longest_line.p1.x,
						y: longest_line.p1.y,
					},
				},
				longest_line_length,
			))
		}
	}

	fn get_debug_view(&self) -> debug::DebugViewImage {
		debug::DebugViewImage::Some(Arc::new(match debug::DebugView::get() {
			debug::DebugView::None => return debug::DebugViewImage::None,
			debug::DebugView::OCRInput => {
				memory!(&self.ocr_out).borrow().as_host_ref().convert()
			},
			debug::DebugView::FindScalesInput => {
				memory!(&self.scales_preprocessed_host).borrow().convert()
			},
			debug::DebugView::LSDPreprocess => {
				image::RgbImage::try_from(memory!(&self.cropped_map)).unwrap().convert()
			},
			debug::DebugView::LSDInput => {
				memory!(&self.lsd_image).borrow().as_host_ref().convert()
			}
		}))
	}
}

#[test]
fn test_gpu_computer_vision() {
	println!("initializing CUDA...");
	let mut cuda = CUDAInstance::init().unwrap();

	let mut i = 0;
	let (cropped_map, lines) = loop {
		let (cropped_map, lines) = {
			println!("decoding sample image...");
			let image = image::load_from_memory(include_bytes!("../../vision-common/samples/point_intersect.png"))
				.unwrap()
				.into_bgra8();

			let image = image::ImageBuffer::from_raw(image.width(), image.height(), image.into_raw().into_boxed_slice()).unwrap();

			let (w, h) = image.dimensions();
			cuda.load_frame(OwnedSubImage::new(image, 0, 0, w, h)).unwrap();
			cuda.load_map_markers(22).unwrap();

			let ui_map = cuda.crop_to_map().unwrap().expect("crop_to_map failed");

			let (_, lines) = rayon::join(
				|| {
					cuda.thread_ctx().unwrap();

					cuda.ocr_preprocess().expect("ocr_preprocess failed");
					cuda.find_scales_preprocess(0).expect("find_scales_preprocess failed");
				},
				|| {
					cuda.thread_ctx().unwrap();

					cuda.isolate_map_markers().expect("isolate_map_markers failed");
					cuda.filter_map_marker_icons().expect("filter_map_marker_icons failed");
					cuda.mask_marker_lines().expect("mask_marker_lines failed");
					cuda.find_marker_lines(22).expect("find_marker_lines failed")
				},
			);

			(ui_map.0, lines)
		};

		if i == 1 {
			break (cropped_map, lines);
		} else {
			i += 1;
		}
	};

	let mut image = image::DynamicImage::ImageRgba8(cropped_map).into_rgb8();
	let len = lines.len();
	for (i, line) in lines.into_iter().enumerate() {
		let f = i as f32 / len as f32;
		let pixel = image::Rgb([((1. - f) * 255.) as u8, (f * 255.) as u8, 0]);
		plot_line(
			&mut image,
			pixel,
			[line.p0.x as u32, line.p0.y as u32],
			[line.p1.x as u32, line.p1.y as u32],
		);
	}
	open_image!(image);
}

#[test]
fn gpu_compute_sanitizer() {
	use std::{
		fmt::Write,
		path::PathBuf,
		process::{Command, Stdio},
	};

	let compute_sanitizer = match which::which("compute-sanitizer") {
		Ok(compute_sanitizer) => compute_sanitizer,
		Err(_) => {
			println!("compute-sanitizer not found, skipping test");
			return;
		}
	};

	println!("compiling test...");

	let mut cargo = Command::new("cargo");
	cargo.args(&[
		"test",
		"test_gpu_computer_vision",
		"--no-run",
		"--quiet",
		"--message-format",
		"json",
		"--manifest-path",
		concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
		"--target-dir",
		concat!(env!("CARGO_MANIFEST_DIR"), "/../target-compute-sanitizer"),
	]);

	#[cfg(not(debug_assertions))]
	{
		cargo.args(&["--release", "--features", "force-gpu-debug,force-gpu-ptx-optimised"]);
	}

	let output = cargo.output().expect("Failed to run cargo test");

	if !output.status.success() {
		panic!(
			"[compute-sanitizer] cargo test error {:?}\n\n===== stdout =====\n{}\n\n===== stderr =====\n{}",
			output.status.code(),
			{
				let stdout = String::from_utf8_lossy(&output.stdout);
				serde_json::from_str(stdout.as_ref())
					.and_then(|json: serde_json::Value| serde_json::to_string_pretty(&json))
					.map(std::borrow::Cow::Owned)
					.unwrap_or(stdout)
			},
			{
				let stderr = String::from_utf8_lossy(&output.stderr);
				serde_json::from_str(stderr.as_ref())
					.and_then(|json: serde_json::Value| serde_json::to_string_pretty(&json))
					.map(std::borrow::Cow::Owned)
					.unwrap_or(stderr)
			}
		);
	}

	#[derive(serde::Deserialize)]
	struct CargoTestOutput {
		executable: PathBuf,
	}

	let mut test_exe = std::str::from_utf8(&output.stdout)
		.expect("cago test json output wasn't valid utf8?")
		.split('\n')
		.filter_map(|msg| serde_json::from_str::<CargoTestOutput>(msg).ok().map(|json| json.executable))
		.collect::<Vec<_>>();

	assert!(!test_exe.is_empty(), "cargo test json output didn't include executable key");
	assert!(test_exe.len() == 1, "cargo test json output included multiple executables");

	let test_exe = test_exe.remove(0);

	println!("using {}", test_exe.display());

	let mut report = String::new();
	["memcheck", "racecheck", "synccheck", "initcheck"].into_iter().for_each(|tool| {
		println!("running {tool}...");

		let output = Command::new(&compute_sanitizer)
			.args(&["--report-api-errors", "all"])
			.args(&["--error-exitcode", "1"])
			.args(&["--tool", tool])
			.arg(&test_exe)
			.arg("test_gpu_computer_vision")
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.output()
			.expect("Failed to run compute-sanitizer");

		if !output.status.success() {
			write!(
				report,
				"####================= {tool} =================####\nstatus: {:?}\n\n===== stdout =====\n{}\n\n===== stderr =====\n{}\n\n",
				output.status.code(),
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr)
			)
			.unwrap();
		}
	});

	if report.is_empty() {
		println!("\n\n[compute-sanitizer] SUCCESS\n\n");
	} else {
		panic!("\n\n{}\n\n", report);
	}
}