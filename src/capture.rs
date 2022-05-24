use crate::prelude::*;

pub struct Frame {
	pub image: VisionFrame,
	pub dpi: Option<u32>
}

magic_statics_mod! {
	static ref FRAME: Mutex<Option<Frame>> = Mutex::new(None);
}

static THREAD_HANDLE: DeferCell<std::thread::Thread> = DeferCell::defer();

#[inline]
pub fn fresh_frame() -> Option<Frame> {
	let frame = FRAME.lock().take();

	if frame.is_none() {
		unpark();
	}

	frame
}

#[inline]
pub fn unpark() {
	if let Some(thread) = THREAD_HANDLE.get() {
		thread.unpark();
	}
}

fn start() {
	let display = scrap::Display::primary().expect("Failed to find primary display!");
	let mut capturer = scrap::Capturer::new(display).expect("Failed to initialize display capturer!");

	// Don't waste time and resources with duplicate frames
	let mut last_frame_crc32 = 0;

	'thread: loop {
		if crate::is_shutdown() {
			break;
		}

		let capture = loop {
			match capturer.frame() {
				Ok(frame) => {
					let crc32 = crc32fast::hash(&frame);
					if last_frame_crc32 != crc32 {
						last_frame_crc32 = crc32;
						break Ok(Box::from(&*frame));
					}
				},

				Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {},

				Err(err) => break Err(err)
			}

			if crate::is_shutdown() {
				break 'thread;
			}

			std::thread::sleep(Duration::from_millis(50)); // 20 Hz

			if crate::is_shutdown() {
				break 'thread;
			}
		};

		match capture {
			Err(err) => {
				log::warn!("Error while capturing frame: {err}");
				std::thread::sleep(Duration::from_millis(50));
			},

			Ok(capture) => {
				let capture = image::ImageBuffer::<image::Bgra<u8>, Box<[u8]>>::from_raw(
					capturer.width() as _,
					capturer.height() as _,
					capture
				).expect("Failed to create image buffer");

				let squadex = squadex::window::get();
				let capture = match squadex.as_ref().and_then(|squadex| squadex.window) {
					Some((x, y, w, h)) => {
						OwnedSubImage::new(capture, x, y, w, h)
					},
					None => {
						let (w, h) = capture.dimensions();
						OwnedSubImage::new(capture, 0, 0, w, h)
					}
				};

				*FRAME.lock() = Some(Frame {
					image: capture,
					dpi: squadex.as_ref().map(|squadex| squadex.dpi)
				});

				if crate::is_shutdown() {
					break 'thread;
				}
				std::thread::park();
			}
		}
	}

	log::info!("capture shutting down...");
}

pub fn spawn() -> JoinHandle<()> {
	let handle = std::thread::spawn(start);
	unsafe { THREAD_HANDLE.set(handle.thread().to_owned()) };
	handle
}