use crate::*;

// checks pixels left, right, up, and down from the given pixel to determine which pixel is the center based on if they're white or black
#[inline]
fn get_centre<I>(image: &I, pt: Point<f32>) -> Point<f32>
where
	I: image::GenericImageView<Pixel = image::Luma<u8>>
{
	const MAX_DIST: f32 = 5.0;

	let mut left = pt.x;
	while left > 0.0
		&& (left - pt.x).abs() < MAX_DIST
		&& unsafe { image.unsafe_get_pixel(left as u32, pt.y as u32)[0] } == 255
	{
		left -= 1.0;
	}

	let mut right = pt.x;
	while right < (image.width() - 1) as f32
		&& (right - pt.x).abs() < MAX_DIST
		&& unsafe { image.unsafe_get_pixel(right as u32, pt.y as u32)[0] } == 255
	{
		right += 1.0;
	}

	let mut up = pt.y;
	while up > 0.0
		&& (up - pt.y).abs() < MAX_DIST
		&& unsafe { image.unsafe_get_pixel(pt.x as u32, up as u32)[0] } == 255
	{
		up -= 1.0;
	}

	let mut down = pt.y;
	while down < (image.height() - 1) as f32
		&& (down - pt.y).abs() < MAX_DIST
		&& unsafe { image.unsafe_get_pixel(pt.x as u32, down as u32)[0] } == 255
	{
		down += 1.0;
	}

	Point::new((left + right) / 2., (up + down) / 2.)
}

#[inline]
fn nearest_point_on_line(pt: Point<f32>, r0: Point<f32>, r1: Point<f32>) -> Point<f32> {
	let dx = r1.x - r0.x;
	let dy = r1.y - r0.y;

	if dx == 0.0 && dy == 0.0 {
		return Point::new(r0.x, r0.y);
	}

	let u = ((pt.x - r0.x) * dx + (pt.y - r0.y) * dy) / (dx * dx + dy * dy);

	Point::new(r0.x + u * dx, r0.y + u * dy)
}

pub fn find_lines<const N: usize, I, IRef: Borrow<I>, FLL, E>(image_ref: &IRef, max_gap: u32, find_longest_line: FLL) -> Result<SmallVec<Line<f32>, N>, E>
where
	I: image::GenericImageView<Pixel = image::Luma<u8>>,
	FLL: Fn(&IRef, Point<f32>, f32) -> Result<(Line<f32>, f32), E>
{
	if N == 0 {
		return Ok(Default::default());
	}

	let image = image_ref.borrow();
	let max_gap = max_gap as f32;

	let mut lines: SmallVec<Line<f32>, N> = SmallVec::new();

	'row: for y in 0..image.height() {
		'column: for x in 0..image.width() {
			let pixel = image.get_pixel_fast(x, y);
			if pixel.0[0] != 255 {
				continue;
			}

			let (x, y) = (x as f32, y as f32);
			let mut pt = Point::new(x, y);

			for line in &lines {
				let nearest_point = nearest_point_on_line(pt, line.p0, line.p1);
				if (x - nearest_point.x).powi(2) + (y - nearest_point.y).powi(2) < 50.0 {
					continue 'column;
				}
			}
			pt = get_centre(image, pt);

			let (mut longest, max_length) = find_longest_line(image_ref, pt, max_gap)?;

			if max_length > 2500.0 {
				longest.p1 = get_centre(image, longest.p1);

				lines.push(longest);

				if lines.len() == N {
					break 'row;
				}
			}
		}
	}

	Ok(lines)
}

// TODO somehow filter out circles