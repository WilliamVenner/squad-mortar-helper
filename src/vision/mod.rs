use smh_vision_common::Vision;
use crate::{prelude::*, capture::Frame};

pub const FPS: u32 = 15;

mod hardware;

mod mpx_ratio;
use mpx_ratio::calc_meters_to_px_ratio;

mod find_minimap;
use find_minimap::find_minimap;

struct DebugWaterfall(*mut Option<Duration>, Instant);
impl Drop for DebugWaterfall {
	#[inline(always)]
	fn drop(&mut self) {
		unsafe { *self.0 = Some(self.1.elapsed()) };
	}
}

#[derive(Default, Debug)]
pub struct VisionResults {
	pub markers: SmallVec<Line<f32>, 32>,
	pub meters_to_px_ratio: Option<f64>,
	pub minimap_bounds: Option<Rect<u32>>,
	pub map: image::RgbaImage,
	pub debug_view: Option<Arc<image::RgbaImage>>
}
pub struct VisionState {
	threads: rayon::ThreadPool,
	find_scales_threads: rayon::ThreadPool,
	find_minimap_threads: rayon::ThreadPool
}
impl VisionState {
	fn process<V: Vision>(&mut self, vision: &mut V, frame: Frame, debug: &mut DebugBox) -> Result<Option<VisionResults>, AnyError>
	where
		AnyError: std::convert::From<V::Error>
	{
		// split the borrow, allowing for the rayon tasks to mutate the DebugBox in parallel!
		let DebugBox { timeshares, ocr: ocr_debug, scales: scales_debug, .. } = debug;

		let (ocr_overlay, scales_overlay) = (SYNCED_DEBUG_STATE.ocr_overlay(), SYNCED_DEBUG_STATE.scales_overlay());

		/*
		DISABLED: see filter_map_marker_icons in this file

		let map_marker_size = 22; // NOTE: this ISN'T scaled to monitor size, the user configures it in their map's sidebar. We assume it's 0.7 (the default) which equals 22px
		vision.load_map_markers(map_marker_size)?;
		*/

		let start = Instant::now();
		let mut result: Result<Option<VisionResults>, AnyError> = (|| {
			macro_rules! debug_waterfall {
				($event:ident => $code:expr) => {{
					let waterfall = DebugWaterfall(&mut timeshares.$event as *mut _, Instant::now());

					let ret = $code;

					drop(waterfall);

					ret
				}};
			}

			debug_waterfall!(load_frame => vision.load_frame(frame.image))?;

			// Crop to the map's bounds on the screen, returning three images:
			// 1. The "UI map", the cropped map but grayscale, which is shown to the user
			// 2. The cropped map but in color, which is used for the actual vision processing
			// 3. The bottom-right quadrant of the map, which is used for the OCR and scales detection
			let (ui_map, [x, y, w, h]) = debug_waterfall!(crop_to_map => match vision.crop_to_map(SETTINGS.grayscale_map())? {
				Some(images) => images,
				None => return Ok(None)
			});

			// brq = bottom-right quadrant
			let (brq_w, brq_h) = (w / 2, h / 2);

			let mut result = VisionResults {
				map: ui_map,
				..Default::default()
			};

			let minimap_bounds = debug_waterfall!(find_minimap => find_minimap(&mut self.find_minimap_threads, vision.get_cpu_frame().view(x, y, w, h)));

			let mut markers = || {
				Ok::<_, AnyError>(if SETTINGS.detect_markers() {
					vision.thread_ctx()?;

					// Isolate green pixels, i.e., squad map markers
					debug_waterfall!(isolate_map_markers => vision.isolate_map_markers())?;

					/*
					DISABLED: Changes to the isolate_map_markers algorithm makes this mostly unnecessary.
					It also doesn't really make sense if there are multiple markers on the map.

					// We will now perform a template match using every map marker type as a template. We need to do this because it messes with the line segment detection.
					// I.e., we want to reduce the amount of points on the image that aren't part of a line.
					// We're lucky because on the Squad map, there will only ever be one green map icon marker, so we can just select the template match with the minimum SAD.
					// Once we've matched this template, we can erase it from the image which will help the line segment detection algorithm with accuracy.
					// However, we will not fully "erase" it, we'll actually leave behind a small square where the map icon marker is pointing to.
					// This will trick the line segment detection algorithm into thinking that the map icon marker is a line segment, connecting the line back up after erasure
					if w >= map_marker_size && h >= map_marker_size {
						debug_waterfall!(filter_map_marker_icons => vision.filter_map_marker_icons())?;
					}
					*/

					// Perform line segment detection on the map to find the map marker lines (i.e. what the player/squad leader is ordering mortar fire on)
					debug_waterfall!(mask_marker_lines => vision.mask_marker_lines())?;

					debug_waterfall!(find_marker_lines => vision.find_marker_lines(
						/* DISABLED ((map_marker_size as f32) * MAP_MARKER_POI_LOCATION) as u32 */
						15
					))?
				} else {
					Default::default()
				})
			};

			let meters_to_px_ratio = if squadex::heightmaps::is_set() {
				// A heightmap is selected, so we can use information from the heightmap to calculate meters instead
				None
			} else {
				Some(|| {
					vision.thread_ctx()?;

					// Use OCR to find the meter scales on the bottom-right quadrant of the map
					let (scales, scales_start_y) = {
						let mut scales_start_y = u32::MAX;
						let mut scales = SmallVec::<_, 3>::new();

						let (ocr_image, ocr_len) = debug_waterfall!(ocr_preprocess => vision.ocr_preprocess())?;
						debug_assert_eq!(ocr_len as u32, brq_w * brq_h);
						let ocr_image = unsafe { core::slice::from_raw_parts(ocr_image, ocr_len) };

						// Telling the OCR engine the DPI of the image improves accuracy
						// `frame.dpi` comes from OS APIs where supported
						// `DPI_ESTIMATE` comes from our window, which might not actually be the same as Squad's DPI, so it's an estimate
						let dpi = frame.dpi.or_else(|| {
							let dpi_estimate = ui::DPI_ESTIMATE.load(std::sync::atomic::Ordering::Relaxed);
							if dpi_estimate == 0 {
								None
							} else {
								Some(dpi_estimate)
							}
						});

						for ocr in debug_waterfall!(ocr => ocr::read(ocr_image, brq_w, brq_h, dpi)).deref() {
							if ocr_overlay {
								ocr_debug.push(ocr::OCRText {
									text: ocr.text.clone(),
									confidence: ocr.confidence,
									left: ocr.left + brq_w,
									top: ocr.top + brq_h,
									right: ocr.right + brq_w,
									bottom: ocr.bottom + brq_h
								});
							}

							if !ocr.text.is_ascii() { continue };

							// Does the text end with an "m"?
							let m = match ocr.text.rfind('m') {
								Some(m) => m,
								None => continue
							};

							// Parse the numerical meters from the text
							let scale = match ocr.text[..m].parse::<u32>() {
								Ok(0) | Err(_) => continue,
								Ok(scale) => scale
							};

							// Take note of the y coordinate of the bottom of this text, we'll crop to this during preprocessing later
							scales_start_y = scales_start_y.min(ocr.bottom);

							// If we've already found this scale, skip it
							if scales.iter().any(|(meters, _)| *meters == scale) { continue };

							scales.push((scale, ((ocr.left + ocr.right) / 2, ocr.bottom)));

							if scales.is_full() {
								break;
							}
						}

						if scales.is_empty() || scales_start_y == u32::MAX {
							return Ok(None);
						}

						(scales, scales_start_y)
					};

					// Crop to the bottom of the first meter scale text we found (scales_start_y)
					let find_scales_image = debug_waterfall!(find_scales_preprocess => vision.find_scales_preprocess(scales_start_y))?;
					let find_scales_image = unsafe { &*find_scales_image }.borrow();

					// Now find the scales themselves, in order to find a meters to pixels ratio.
					// The scales are horizontal black lines with vertical black lines on the start and end.
					// Like this: |----------------|
					Ok::<_, AnyError>(if scales_overlay {
						let meters_to_px_ratio = debug_waterfall!(calc_meters_to_px_ratio => calc_meters_to_px_ratio(&mut self.find_scales_threads, scales, &*find_scales_image, Some(scales_debug)));

						scales_debug.iter_mut().for_each(|(_, scale)| {
							scale.p0.x += brq_w;
							scale.p0.y += brq_h;
							scale.p1.x += brq_w;
							scale.p1.y += brq_h;
						});

						meters_to_px_ratio
					} else {
						debug_waterfall!(calc_meters_to_px_ratio => calc_meters_to_px_ratio(&mut self.find_scales_threads, scales, &*find_scales_image, None))
					})
				})
			};

			let (markers, meters_to_px_ratio) = if let Some(meters_to_px_ratio) = meters_to_px_ratio {
				self.threads.join(markers, meters_to_px_ratio)
			} else {
				(markers(), Ok(None))
			};

			result.minimap_bounds = minimap_bounds;
			result.markers = markers?;
			result.meters_to_px_ratio = meters_to_px_ratio?;

			Ok(Some(result))
		})();

		timeshares.entire_frame = Some(start.elapsed());

		// If the user has selected a debug view, override the map with that
		if let Ok(Some(ref mut result)) = result {
			result.debug_view = vision.get_debug_view(SYNCED_DEBUG_STATE.debug_view());
		}

		result
	}
}

pub fn start() {
	let mut hardware = hardware::init();

	let mut state = VisionState {
		threads: rayon::ThreadPoolBuilder::new().num_threads(4).build().expect("Failed to create rayon thread pool"),
		find_scales_threads: rayon::ThreadPoolBuilder::new().num_threads(3).build().expect("Failed to create rayon thread pool"),
		find_minimap_threads: rayon::ThreadPoolBuilder::new().num_threads(4).build().expect("Failed to create rayon thread pool")
	};

	let fps_interval = Duration::from_secs_f32(1.0 / FPS as f32);
	loop {
		if crate::is_shutdown() {
			break;
		}

		while SETTINGS.paused() {
			std::thread::park();

			if crate::is_shutdown() {
				break;
			}
		}

		if let Some(frame) = ui::debug::FakeInputs::selected().map(|image| Frame { dpi: None, image }).or_else(capture::fresh_frame) {
			let last_frame = Instant::now();

			let mut debug_box = DebugBox::default();

			let vision = {
				let vision = hardware.process(&mut state, &mut debug_box, frame);
				if let Err(err) = vision.as_ref() {
					log::warn!("Error processing frame: {err}\n{}", err.backtrace());
				}
				vision.ok().flatten()
			};

			ui::update(|ui_data| {
				ui_data.debug = debug_box;

				let vision = match vision {
					Some(vision) => vision,
					None => {
						ui_data.sleeping = true;
						return;
					}
				};

				ui_data.sleeping = false;

				ui_data.map = Arc::new(vision.map);

				ui_data.meters_to_px_ratio = vision.meters_to_px_ratio;

				ui_data.minimap_bounds = vision.minimap_bounds;

				ui_data.debug.debug_view = vision.debug_view;

				ui_data.markers = vision.markers.into_iter().map(|Line { p0, p1 }| {
					ui::Marker::new(p0.into(), p1.into(), vision.meters_to_px_ratio)
				}).collect::<Box<_>>();
			});

			let last_frame = last_frame.elapsed();
			if last_frame < fps_interval {
				std::thread::sleep(fps_interval - last_frame);
			}
		} else {
			std::thread::sleep(fps_interval);
		}
	}

	log::info!("vision shutting down...");
}