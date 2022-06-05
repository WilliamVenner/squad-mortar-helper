use super::*;

type MinimapFrame<'a> = image::SubImage<&'a image::ImageBuffer<image::Bgra<u8>, Box<[u8]>>>;

// TODO "fuzz" test this to find oob

#[inline]
/// Returns the maximum absolute difference between all the pixels surrounding the given pixel divided by 255 * 3
pub fn get_edginess<I>(image: &I, x: u32, y: u32) -> f32
where
	I: image::GenericImageView<Pixel = image::Bgra<u8>>
{
	debug_assert!(x > 0 && y > 0 && x < image.width() - 1 && y < image.height() - 1, "x and y need to have 1px space around them");

	let perimeter = [
		(x - 1, y - 1),
		(x, y - 1),
		(x + 1, y - 1),

		(x - 1, y + 1),
		(x, y + 1),
		(x + 1, y + 1),

		(x - 1, y),
		(x + 1, y),
	];

	let pixel = image.get_pixel_fast(x, y);

	let max = perimeter
		.into_iter()
		.map(|(px, py)| {
			let ppixel = image.get_pixel_fast(px, py);
			pixel
				.0
				.into_iter()
				.take(3)
				.zip(ppixel.0.into_iter().take(3))
				.map(|(a, b)| a.abs_diff(b) as u16)
				.sum::<u16>()
		})
		.max()
		.unwrap();

	max as f32 / 765.0
}

pub fn find_minimap(threads: &mut rayon::ThreadPool, frame: MinimapFrame) -> Option<Rect<u32>> {
	let (w, h) = frame.dimensions();
	if w < 3 || h < 3 {
		return None;
	};

	// Start from the center, drawing a cross outwards until we find the edge

	#[derive(Clone, Copy, Debug, PartialEq, Eq)]
	enum Direction {
		Up,
		Down,
		Left,
		Right
	}

	fn find_edge(image: &MinimapFrame, x: u32, y: u32, dir: Direction) -> Option<u32> {
		const EDGINESS_THRESHOLD: f32 = 0.01;

		let mut xy = [x, y];

		// c = component we're changing
		// c_max = maximum value of c (width or height of image)
		// oc = other component
		// oc_max = maximum value of oc (width or height of image)
		// cod = component/other component delta every iteration
		let (c, mut c_max, oc, mut oc_max, cod) = match dir {
			Direction::Up => (1, image.height(), 0, image.width(), -1),
			Direction::Down => (1, image.height(), 0, image.width(), 1),
			Direction::Left => (0, image.width(), 1, image.height(), -1),
			Direction::Right => (0, image.width(), 1, image.height(), 1)
		};

		let min_line_length = (oc_max.abs_diff(xy[oc]) / 2) - 1;

		c_max -= 3;
		oc_max -= 3;

		'find_edge: loop {
			{
				let c = &mut xy[c];

				*c = (*c as i32 + cod) as u32;

				if *c > c_max {
					return Some(c_max + 2);
				} else if *c < 3 {
					return Some(0);
				}
			}

			// Find pixel under the edginess threshold
			if get_edginess(image, xy[0], xy[1]) <= EDGINESS_THRESHOLD {
				// Try and find a straight line of pixels that are also under the edginess threshold
				let ret = xy[c];
				let mut xy = xy;

				let mut min_line_length = min_line_length;
				while min_line_length > 0 {
					{
						let oc = &mut xy[oc];

						*oc = (*oc as i32 - cod) as u32;

						if *oc < 3 || *oc > oc_max {
							continue 'find_edge;
						}
					}

					if get_edginess(image, xy[0], xy[1]) <= EDGINESS_THRESHOLD {
						min_line_length -= 1;
					} else {
						continue 'find_edge;
					}
				}

				return Some((ret as i32 - cod) as u32);
			}
		}
	}

	let (x, y) = (w / 2, h / 2);
	let (left, right, top, bottom) = rayon_join_all!(threads => {
		left: || find_edge(&frame, x, y, Direction::Left),
		right: || find_edge(&frame, x, y, Direction::Right),
		up: || find_edge(&frame, x, y, Direction::Up),
		down: || find_edge(&frame, x, y, Direction::Down),
	});

	debug_assert!(right.is_none() || right.unwrap() < w, "right {right:?} is larger than width {w}");
	debug_assert!(bottom.is_none() || bottom.unwrap() < h, "bottom {bottom:?} is larger than height {h}");

	Some(Rect {
		left: left?,
		right: right?,
		top: top?,
		bottom: bottom?
	})
}

#[test]
fn test_edginess() {
	let image = image::load_from_memory(include_bytes!("../../vision-common/samples/fullmapgreen.jpg")).unwrap().into_bgra8();
	let mut out = image::GrayImage::new(image.width(), image.height());

	let par_image = UnsafeSendPtr::new_const(&image);
	let par_out = UnsafeSendPtr::new_mut(&mut out);
	par_iter_pixels!(image).for_each(|(x, y, _)| {
		let image = unsafe { par_image.clone().as_const() };
		let out = unsafe { par_out.clone().as_mut() };

		if x < 3 || x > image.width() - 3 || y < 3 || y > image.height() - 3 {
			return;
		}

		let edginess = (get_edginess(image, x, y) * 255.0) as u8;
		out.put_pixel_fast(x, y, image::Luma([edginess]));
	});

	open_image!(out);
}
