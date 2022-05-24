#[allow(unused)]
#[doc(hidden)]
pub static OPEN_IMAGE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[macro_export]
#[allow(unused)]
macro_rules! open_image {
	($image:expr) => {{
		let image = $image;

		let id = $crate::OPEN_IMAGE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

		let path = std::env::temp_dir().join(&format!("temp{id}.png"));
		image.save_with_format(&path, image::ImageFormat::Png).expect("Failed to save image");
		$crate::open::that(path).expect("Failed to open image");

		image
	}};
}

#[macro_export]
macro_rules! timed_progress {
	([$macro:ident!] => $code:block) => {{
		let mut total_time = std::time::Duration::ZERO;
		macro_rules! $macro {
			($msg:literal, $op:expr) => {{
				print!($msg);
				print!(" ");
				let _ = std::io::Write::flush(&mut std::io::stdout());
				let start = std::time::Instant::now();
				let ret = $op;
				let elapsed = start.elapsed();
				print!("{:?}", elapsed);
				total_time += elapsed;
				println!();
				ret
			}}
		}

		let ret = $code;

		println!("total time: {:?} ({} FPS)", total_time, 1.0 / total_time.as_secs_f64());

		ret
	}}
}

#[allow(unused)]
pub fn plot_line<I: image::GenericImage>(img: &mut I, px: I::Pixel, p0: [u32; 2], p1: [u32; 2]) {
	let (x0, y0, x1, y1) = (p0[0], p0[1], p1[0], p1[1]);
	let (mut x0, mut y0, x1, y1) = (x0 as i32, y0 as i32, x1 as i32, y1 as i32);
	let dx = (x1 as i64 - x0 as i64).abs() as i32;
	let sx = if x0 < x1 { 1 } else { -1 };
	let dy = -((y1 as i64 - y0 as i64).abs() as i32);
	let sy = if y0 < y1 { 1 } else { -1 };
	let mut error = dx + dy;

	loop {
		img.put_pixel(x0.try_into().unwrap(), y0.try_into().unwrap(), px);
		if x0 == x1 && y0 == y1 { break };
		let e2 = 2 * error;
		if e2 >= dy {
			if x0 == x1 { break };
			error += dy;
			x0 += sx;
		}
		if e2 <= dx {
			if y0 == y1 { break };
			error += dx;
			y0 += sy;
		}
	}
}