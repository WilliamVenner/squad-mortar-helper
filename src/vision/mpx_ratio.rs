use super::*;

pub fn calc_meters_to_px_ratio(threads: &mut rayon::ThreadPool, scales: SmallVec<(u32, (u32, u32)), 3>, image: &image::GrayImage, scales_debug: Option<&mut SmallVec<(u32, Line<u32>), 3>>) -> Option<f64> {
	fn find_scale_width(meters: u32, (x, y): (u32, u32), image: &image::GrayImage, scales_debug: Option<&mut Option<(u32, Line<u32>)>>) -> Option<f64> {
		const MIN_SCALE_WIDTH: u32 = 10;
		const MIN_SCALE_VERTICAL_BAR_HEIGHT: u32 = 4;

		if y < MIN_SCALE_VERTICAL_BAR_HEIGHT {
			return None;
		}

		let max_scale_y_offset = ((20.0 / 640.0) * image.width() as f64).round() as u32;

		// Go down...
		'y: for y in y..image.height().min(y + max_scale_y_offset) {
			let pixel = image.get_pixel_fast(x, y);

			if pixel.0[0] == 0 {
				// Go right...
				let mut right = 0;
				'right: for x in x..image.width() {
					let pixel = image.get_pixel_fast(x, y);
					if pixel.0[0] != 0 {
						let x = x - 1;

						// Make sure this is the vertical line upwards
						for y in (y..y + MIN_SCALE_VERTICAL_BAR_HEIGHT).chain((y..y - MIN_SCALE_VERTICAL_BAR_HEIGHT).rev()) {
							let pixel = image.get_pixel_fast(x, y);
							if pixel.0[0] != 0 {
								continue 'right;
							}
						}

						right = x;
						break;
					}
				}
				if right == 0 {
					continue 'y;
				}

				// Go left...
				let mut left = 0;
				'left: for x in (0..x).rev() {
					let pixel = image.get_pixel_fast(x, y);
					if pixel.0[0] != 0 {
						let x = x + 1;

						// Make sure this is the vertical line upwards
						for y in (y..y + MIN_SCALE_VERTICAL_BAR_HEIGHT).chain((y..y - MIN_SCALE_VERTICAL_BAR_HEIGHT).rev()) {
							let pixel = image.get_pixel_fast(x, y);
							if pixel.0[0] != 0 {
								continue 'left;
							}
						}

						left = x;
						break;
					}
				}
				if left == 0 {
					continue 'y;
				}

				let width = (right - left) + 1;
				if width < MIN_SCALE_WIDTH {
					continue 'y;
				}

				if let Some(scales_debug) = scales_debug {
					*scales_debug = Some((meters, Line::new(
						Point::new(left, y),
						Point::new(right, y),
					)));
				}

				return Some(meters as f64 / width as f64);
			}
		}

		None
	}

	let scales = scales.as_slice();
	if scales.is_empty() {
		return None
	}

	let (mut a, mut b, mut c) = (None, None, None);
	let (a_opt, b_opt, c_opt) = if scales_debug.is_none() {
		(None, None, None)
	} else {
		(Some(&mut a), Some(&mut b), Some(&mut c))
	};

	// I call it the "Rayon ladder", it's cursed and disgusting but faster than a parallel iterator, avoids heap allocation and compiles to a surprisingly little number of instructions
	let ret = match scales.len() {
		1 => find_scale_width(scales[0].0, scales[0].1, image, a_opt),
		2 => {
			match threads.join(
				|| find_scale_width(scales[0].0, scales[0].1, image, a_opt),
				|| find_scale_width(scales[1].0, scales[1].1, image, b_opt),
			) {
				(Some(a), Some(b)) => Some((a + b) / 2.0),
				(Some(a), None) => Some(a),
				(None, Some(b)) => Some(b),

				(None, None) => None
			}
		},
		3 => match threads.join(
			|| find_scale_width(scales[0].0, scales[0].1, image, a_opt),
			|| threads.join(
				|| find_scale_width(scales[1].0, scales[1].1, image, b_opt),
				|| find_scale_width(scales[2].0, scales[2].1, image, c_opt)
			)
		) {
			(Some(a), (Some(b), Some(c))) => Some((a + b + c) / 3.0),

			(Some(a), (Some(b), None)) => Some((a + b) / 2.0),
			(Some(a), (None, Some(c))) => Some((a + c) / 2.0),
			(None, (Some(b), Some(c))) => Some((b + c) / 2.0),

			(Some(a), (None, None)) => Some(a),
			(None, (Some(b), None)) => Some(b),
			(None, (None, Some(c))) => Some(c),

			(None, (None, None)) => None
		},
		_ => unreachable!()
	};

	if let Some(scales_debug) = scales_debug {
		[a, b, c].into_iter().flatten().for_each(|scale| {
			scales_debug.push(scale);
		});
	}

	ret
}