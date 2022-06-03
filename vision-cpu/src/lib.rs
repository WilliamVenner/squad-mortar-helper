#[allow(unused)]
use smh_vision_common::open_image;

use smh_vision_common::{
	consts::*,
	markers,
	prelude::{image::Pixel, *},
	Vision,
};

#[derive(Default)]
pub struct CPUFallback {
	frame: Arc<VisionFrame>,

	cropped_map: SusRefCell<image::RgbImage>,
	cropped_brq: SusRefCell<image::RgbImage>,

	ocr_out: SusRefCell<image::GrayImage>,

	scales_preprocessed: SusRefCell<image::GrayImage>,

	lsd_image: SusRefCell<image::GrayImage>,

	marked_marker_pixels: (SusRefCell<Vec<(u32, u32)>>, AtomicUsize),
	markers: [Box<[markers::MapMarkerPixel]>; markers::AMOUNT],
	map_marker_size: u32,
}

macro_rules! memory {
	(&$self:ident.$item:ident) => {
		$self.$item.borrow()
	};

	(&mut $self:ident.$item:ident) => {
		$self.$item.borrow_mut()
	};
}

#[inline]
pub fn ocr_brightness(pixel: image::Rgb<u8>) -> u8 {
	pixel.0[0].min(pixel.0[1]).min(pixel.0[2])
}

#[inline]
pub fn ocr_monochromaticy(pixel: image::Rgb<u8>) -> u16 {
	let mut diff: u16 = 0;
	for a in 0..3 {
		for b in 0..3 {
			diff += pixel[a].abs_diff(pixel[b]) as u16;
		}
	}
	diff
}

impl Vision for CPUFallback {
	type LSDImage = image::GrayImage;
	type Error = std::convert::Infallible;

	#[inline]
	fn init() -> Result<Self, AnyError> {
		Ok(CPUFallback::default())
	}

	#[inline]
	fn thread_ctx(&self) -> Result<(), AnyError> {
		Ok(())
	}

	fn load_frame(&mut self, frame: VisionFrame) -> Result<(), Self::Error> {
		if self.frame.dimensions() != frame.dimensions() {
			let [_, _, w, h] = MAP_BOUNDS.into_absolute([frame.width(), frame.height()]);

			// Map fills remaining space
			let w = frame.width() - w;

			// brq = bottom right quadrant
			let brq_w = w / 2;
			let brq_h = h / 2;

			// todo we can do this in-place to save resources when map is closed

			self.cropped_map = image::RgbImage::new(w, h).into();
			self.cropped_brq = image::RgbImage::new(brq_w, brq_h).into();
			self.ocr_out = image::GrayImage::new(brq_w, brq_h).into();
			self.marked_marker_pixels = (SusRefCell::new(vec![Default::default(); w as usize * h as usize]), AtomicUsize::new(0));
			self.scales_preprocessed = image::GrayImage::new(brq_w, brq_h).into();
			self.lsd_image = image::GrayImage::new(w, h).into();
		}

		self.frame = Arc::new(frame);

		Ok(())
	}

	#[inline]
	fn get_cpu_frame(&self) -> Arc<VisionFrame> {
		self.frame.clone()
	}

	fn load_map_markers(&mut self, map_marker_size: u32) -> Result<(), Self::Error> {
		if self.map_marker_size != map_marker_size {
			self.markers = markers::load_markers::<markers::FilteredMarkers>(map_marker_size);
			self.map_marker_size = map_marker_size;
		}
		Ok(())
	}

	fn crop_to_map(&self, grayscale: bool) -> Result<Option<(image::RgbaImage, [u32; 4])>, Self::Error> {
		let frame = &self.frame;
		let mut cropped_map = memory!(&mut self.cropped_map);
		let mut cropped_brq = memory!(&mut self.cropped_brq);

		{
			let [x, y, w, h] = CLOSE_DEPLOYMENT_BUTTON_BOUNDS.into_absolute([frame.width(), frame.height()]);

			// Find the red "Close Deployment" button
			// TODO: support the quick map as well
			let red_pixels = par_iter_pixels!(frame[x, y, w, h])
				.filter(|(_, _, pixel)| {
					for (i, pixel) in pixel.to_rgb().0.into_iter().enumerate() {
						if (CLOSE_DEPLOYMENT_BUTTON_COLOR[i] - pixel as i16).abs() as u16 > CLOSE_DEPLOYMENT_BUTTON_TOLERANCE {
							return false;
						}
					}
					true
				})
				.count();

			let red_pixels = red_pixels as f32 / (w * h) as f32;
			if red_pixels < CLOSE_DEPLOYMENT_BUTTON_RED_PIXEL_THRESHOLD {
				return Ok(None);
			}
		}

		let [x, y, w, h] = MAP_BOUNDS.into_absolute([frame.width(), frame.height()]);

		// Map fills remaining space
		let w = frame.width() - w;
		let x = frame.width() - x - w;

		// brq = bottom right quadrant
		let brq_w = w / 2;
		let brq_h = h / 2;

		let (ui_map, _, _) = rayon_join_all! {
			a: || {
				let mut ui_map = image::RgbaImage::new(w, h);
				let par_ui_map = UnsafeSendPtr::new_mut(&mut ui_map);
				if grayscale {
					par_iter_pixels!(frame[x, y, w, h]).for_each(|(image_x, image_y, bgra)| {
						let ui_map = unsafe { par_ui_map.clone().as_mut() };
						let luma8 = bgra.to_luma().0[0];
						ui_map.put_pixel_fast(image_x - x, image_y - y, image::Rgba([luma8, luma8, luma8, 255]));
					});
				} else {
					par_iter_pixels!(frame[x, y, w, h]).for_each(|(image_x, image_y, bgra)| {
						let ui_map = unsafe { par_ui_map.clone().as_mut() };
						ui_map.put_pixel_fast(image_x - x, image_y - y, image::Rgba([bgra.0[2], bgra.0[1], bgra.0[0], 255]));
					});
				}
				ui_map
			},

			b: || frame.par_crop_into(x, y, w, h, &mut *cropped_map),
			c: || frame.par_crop_into(x + brq_w, y + brq_h, brq_w, brq_h, &mut *cropped_brq),
		};

		Ok(Some((ui_map, [x, y, w, h])))
	}

	fn ocr_preprocess(&self) -> Result<(*const u8, usize), Self::Error> {
		let cropped_brq = memory!(&self.cropped_brq);
		let mut ocr_out = memory!(&mut self.ocr_out);

		let (w, h) = cropped_brq.dimensions();

		let par_cropped_brq = UnsafeSendPtr::new_const(&*cropped_brq);
		let par_ocr_out = UnsafeSendPtr::new_mut(&mut *ocr_out);
		(0..w)
			.into_par_iter()
			.map(|x| (0..h).into_par_iter().map(move |y| (x, y)))
			.flatten()
			.for_each(|(x, y)| {
				let cropped_brq = unsafe { par_cropped_brq.clone().as_const() };
				let ocr_out = unsafe { par_ocr_out.clone().as_mut() };

				let pixel = cropped_brq.get_pixel_fast(x, y);

				// If the pixel passes OCR_PREPROCESS_MONOCHROMATICY_THRESHOLD and OCR_PREPROCESS_BRIGHTNESS_THRESHOLD then OK
				// If the pixel doesn't pass, but has a nearby pixel that does, and itself passes OCR_PREPROCESS_MONOCHROMATICY_THRESHOLD, OCR_PREPROCESS_BRIGHTNESS_EDGE_THRESHOLD, then OK

				let should_keep = || {
					let diff = ocr_monochromaticy(pixel);
					if diff <= OCR_PREPROCESS_MONOCHROMATICY_THRESHOLD && pixel.0.into_iter().all(|px| px >= OCR_PREPROCESS_BRIGHTNESS_THRESHOLD) {
						return true;
					} else if diff <= OCR_PREPROCESS_SIMILARITY_EDGE_THRESHOLD
						&& pixel.0.into_iter().all(|px| px >= OCR_PREPROCESS_BRIGHTNESS_EDGE_THRESHOLD)
					{
						for xx in x.saturating_sub(OCR_PREPROCESS_DILATE_RADIUS)
							..=x.saturating_add(OCR_PREPROCESS_DILATE_RADIUS).min(w - OCR_PREPROCESS_DILATE_RADIUS)
						{
							for yy in y.saturating_sub(OCR_PREPROCESS_DILATE_RADIUS)
								..=y.saturating_add(OCR_PREPROCESS_DILATE_RADIUS).min(h - OCR_PREPROCESS_DILATE_RADIUS)
							{
								let pixel = cropped_brq.get_pixel_fast(xx, yy);

								if pixel.0.into_iter().any(|px| px < OCR_PREPROCESS_BRIGHTNESS_THRESHOLD) {
									continue;
								}

								if ocr_monochromaticy(pixel) <= OCR_PREPROCESS_MONOCHROMATICY_THRESHOLD {
									return true;
								}
							}
						}
					}

					false
				};

				if should_keep() {
					ocr_out.put_pixel_fast(x, y, image::Luma([255 - pixel.to_luma().0[0]]));
				} else {
					ocr_out.put_pixel_fast(x, y, image::Luma([255]));
				}
			});

		Ok((ocr_out.as_ptr(), ocr_out.len()))
	}

	fn find_scales_preprocess(&self, scales_start_y: u32) -> Result<*const SusRefCell<image::GrayImage>, Self::Error> {
		let cropped_brq = memory!(&self.cropped_brq);
		let mut scales_preprocessed = memory!(&mut self.scales_preprocessed);

		let (w, h) = cropped_brq.dimensions();

		let par_scales_preprocessed = UnsafeSendPtr::new_mut(&mut *scales_preprocessed);
		par_iter_pixels!(cropped_brq[0, scales_start_y, w, h - scales_start_y]).for_each(|(x, y, pixel)| {
			let scales_preprocessed = unsafe { par_scales_preprocessed.clone().as_mut() };
			let pixel = if pixel.to_luma().0[0] != 0 {
				image::Luma([255])
			} else {
				image::Luma([0])
			};
			scales_preprocessed.put_pixel_fast(x, y, pixel);
		});

		Ok(&self.scales_preprocessed as *const _)
	}

	fn isolate_map_markers(&self) -> Result<(), Self::Error> {
		let mut cropped_map = memory!(&mut self.cropped_map);

		let marked_marker_pixels = memory!(&self.marked_marker_pixels);
		let (mut marked_marker_pixels, len) = (marked_marker_pixels.0.borrow_mut(), &marked_marker_pixels.1);

		len.store(0, std::sync::atomic::Ordering::Release);

		// Isolate green pixels
		let par_cropped_map = UnsafeSendPtr::new_mut(&mut *cropped_map);
		let par_marked_marker_pixels = UnsafeSendPtr::new_mut(&mut *marked_marker_pixels);
		par_iter_pixels!(cropped_map).for_each(move |(x, y, pixel)| {
			let cropped_map = unsafe { par_cropped_map.clone().as_mut() };
			let marked_marker_pixels = unsafe { par_marked_marker_pixels.clone().as_mut() };

			if !markers::is_any_map_marker_color(pixel) {
				cropped_map.put_pixel_fast(x, y, image::Rgb([0, 0, 0]));
			} else if x >= self.map_marker_size
				&& y >= self.map_marker_size
				&& x < cropped_map.width() - self.map_marker_size - 1
				&& y < cropped_map.height() - self.map_marker_size - 1
			{
				marked_marker_pixels[len.fetch_add(1, std::sync::atomic::Ordering::SeqCst)] = (x, y);
			}
		});

		Ok(())
	}

	fn filter_map_marker_icons(&self) -> Result<(), Self::Error> {
		let map_marker_size = self.map_marker_size;
		let markers = &self.markers;
		let mut cropped_map = memory!(&mut self.cropped_map);

		let marked_marker_pixels = memory!(&self.marked_marker_pixels);
		let marked_marker_pixels = &marked_marker_pixels.0.borrow()[0..marked_marker_pixels.1.load(std::sync::atomic::Ordering::Acquire)];

		#[derive(Clone, Copy)]
		struct TemplateMatch {
			x: u32,
			y: u32,
			sad: u32,
		}
		impl std::fmt::Debug for TemplateMatch {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.debug_struct("TemplateMatch")
					.field("x", &self.x)
					.field("y", &self.y)
					.field("sad", &self.sad)
					.finish()
			}
		}

		let template_match = marked_marker_pixels
			.par_iter()
			.copied()
			.map(|(x, y)| markers.par_iter().map(move |marker| (x, y, marker)))
			.flatten()
			.map(|(x, y, marker)| {
				let mut sad: u32 = 0;

				for marker in marker.iter() {
					let cropped_map = cropped_map.get_pixel(marker.x + x, marker.y + y); // FIXME panic here?!
					let alpha = marker.pixel.0[3];

					let ad = cropped_map
						.0
						.into_iter()
						.zip(marker.pixel.0.into_iter().take(3))
						.map(|(a, b)| a.abs_diff(b) as u32)
						.sum::<u32>();

					let ad = ((ad as f32) * (alpha as f32 / 255.0)) as u32; // alpha blending - transparent pixels should have less impact on the overall absolute difference
					sad += ad;
				}

				TemplateMatch { x, y, sad }
			})
			.min_by_key(|template| template.sad);

		if let Some(TemplateMatch { x, y, .. }) = template_match {
			// Erase the marker icon from the map!
			for conv_x in x..(x + map_marker_size).min(cropped_map.width() - 1) {
				for conv_y in y..(y + map_marker_size).min(cropped_map.height() - 1) {
					cropped_map.put_pixel_fast(conv_x, conv_y, image::Rgb([0, 0, 0]));
				}
			}

			// Trick the line segment detection algorithm into continuing the line by placing a 4x4 square where the marker icon was pointing
			// It should hopefully fill the gap and continue the line
			let x = x + (map_marker_size / 2);
			let y = y + (map_marker_size as f32 * MAP_MARKER_POI_LOCATION).round() as u32;
			for conv_x in x.saturating_sub(2)..(x + 2) {
				for conv_y in y.saturating_sub(2)..(y + 2) {
					cropped_map.put_pixel_fast(conv_x, conv_y, image::Rgb([0, 255, 0]));
				}
			}
		}

		Ok(())
	}

	fn mask_marker_lines(&self) -> Result<(), Self::Error> {
		let cropped_map = memory!(&self.cropped_map);
		let mut lsd_image = memory!(&mut self.lsd_image);

		let par_lsd_image = UnsafeSendPtr::new_mut(&mut *lsd_image);
		par_iter_pixels!(cropped_map).for_each(move |(x, y, pixel)| {
			let lsd_image = unsafe { par_lsd_image.clone().as_mut() };

			if markers::is_any_map_marker_color(pixel) {
				lsd_image.put_pixel_fast(x, y, image::Luma([255]));
			} else {
				lsd_image.put_pixel_fast(x, y, image::Luma([0]));
			}
		});

		imageproc::morphology::dilate_mut(&mut *lsd_image, imageproc::distance_transform::Norm::L1, 1);

		Ok(())
	}

	fn find_marker_lines(&self, max_gap: u32) -> Result<SmallVec<Line<f32>, 32>, Self::Error> {
		let lsd_image = memory!(&self.lsd_image);
		lsd::find_lines(
			&*lsd_image,
			max_gap,
			#[inline]
			|image, pt, max_gap| self.find_longest_line(image, pt, max_gap),
		)
	}

	fn find_longest_line(&self, image: &Self::LSDImage, pt: Point<f32>, max_gap: f32) -> Result<(Line<f32>, f32), Self::Error> {
		let find_line_in_image = |pt: Point<f32>, max_gap: f32, theta: f32| {
			let (mut x, mut y) = (pt.x, pt.y);

			let x_start = x;
			let y_start = y;
			let mut x_end = x;
			let mut y_end = y;

			let mut gap = (0.0, 0.0, 0.0);

			let dx = theta.cos();
			let dy = theta.sin();
			let mut x_offset = 0.0;
			let mut y_offset = 0.0;

			while x >= 0.0 && y >= 0.0 && x < image.width() as f32 && y < image.height() as f32 {
				if unsafe { image.unsafe_get_pixel(x as u32, y as u32) }[0] == 255 {
					// there's no gap, reset state
					gap = (0.0, 0.0, 0.0);
				} else if gap.0 >= max_gap {
					// gap didn't close, abort
					// restore saved state
					(x, y) = (gap.1, gap.2);
					break;
				} else if gap.0 == 0.0 {
					// save the state of (x, y) so we can restore it later if the gap isn't closed
					gap = (1.0, x, y);
				} else {
					// keep going in case there is a gap that closes
					gap.0 += 1.0;
				}

				x_offset += dx;
				y_offset += dy;
				x = x_offset + x_start;
				y = y_offset + y_start;
			}

			if image.get_pixel_checked(x as u32, y as u32) == Some(image::Luma([0])) {
				x_end = x - dx;
				y_end = y - dy;
			}

			Line::new(Point::new(x_start, y_start), Point::new(x_end, y_end))
		};

		let (longest, length) = (0..3600_u32)
			.into_par_iter()
			.map(|theta| {
				let line = find_line_in_image(pt, max_gap, ((theta as f32) / 10.0).to_radians());
				(line, line.p0.distance_sqr(&line.p1))
			})
			.reduce(Default::default, |(a_line, a_length), (b_line, b_length)| {
				if a_length > b_length {
					(a_line, a_length)
				} else {
					(b_line, b_length)
				}
			});

		Ok((longest, length))
	}

	fn get_debug_view(&self, choice: debug::DebugView) -> Option<Arc<image::RgbaImage>> {
		Some(Arc::new(match choice {
			debug::DebugView::None => return None,
			debug::DebugView::OCRInput => self.ocr_out.borrow().convert(),
			debug::DebugView::FindScalesInput => self.scales_preprocessed.borrow().convert(),
			debug::DebugView::LSDPreprocess => self.cropped_map.borrow().convert(),
			debug::DebugView::LSDInput => self.lsd_image.borrow().convert(),
			debug::DebugView::CroppedBRQ => self.cropped_brq.borrow().convert(),
		}))
	}
}