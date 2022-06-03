//! On unsupported operating systems, use scrap

use crate::prelude::*;

thread_local! {
	static CAPTURER: (u32, u32, RefCell<scrap::Capturer>) = {
		let display = scrap::Display::primary().expect("Failed to find primary display");
		(display.width() as u32, display.height() as u32, RefCell::new(scrap::Capturer::new(display).expect("Failed to create capturer")))
	};
}

pub fn frame() -> Result<image::ImageBuffer<image::Bgra<u8>, Box<[u8]>>, anyhow::Error> {
	CAPTURER.with(|(w, h, capturer)| {
		let mut capturer = capturer.borrow_mut();
		let frame = capturer.frame()?;
		Ok::<_, anyhow::Error>(image::ImageBuffer::from_raw(*w, *h, Box::from(&*frame)).sus_unwrap())
	})
}