#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(windows, target_feature = "crt-static"))]
compile_error!("Should be built without +crt-static to reduce binary size. Squad already makes users install MSVCRT 2015");

#[macro_use]
extern crate magic_static;

#[macro_use]
extern crate lazy_static;

mod capture;
mod settings;
mod squadex;
mod ui;
mod vision;

use prelude::*;
mod prelude {
	pub(crate) use crate::{
		ui::debug::{SYNCED_DEBUG_STATE, DebugBox},
		capture,
		settings::SETTINGS,
		squadex, ui, vision,
	};
	pub(crate) use smh_vision_common::prelude::*;
	pub(crate) use smh_vision_ocr as ocr;
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static JOIN_MAIN_THREAD: DeferCell<std::sync::mpsc::Receiver<()>> = DeferCell::defer();

fn main() {
	std::env::set_var("RUST_BACKTRACE", "full");

	smh_vision_common::dylib::panic_hook();

	let logger = ui::logs::init();

	magic_static::init! {
		mod settings,
		mod capture,
		mod ui
	};

	let shutdown_tx = {
		let (tx, rx) = std::sync::mpsc::sync_channel(1);
		unsafe { JOIN_MAIN_THREAD.set(rx) };
		tx
	};

	if ctrlc::set_handler(ctrlc_shutdown).is_err() {
		log::error!("Failed to set CTRL+C handler, shutting down might not work");
	}

	let vision = std::thread::Builder::new()
		.stack_size(10 * 1024 * 1024) // We like the stack.
		.name("vision".to_string())
		.spawn(vision::start)
		.expect("Failed to spawn vision thread");

	let capture = capture::spawn();

	ui::start(vision.thread().to_owned(), logger, move || {
		SHUTDOWN.store(true, std::sync::atomic::Ordering::Release);

		vision.thread().unpark();

		for thread in [capture, vision].into_iter() {
			if let Err(err) = thread.join() {
				if let Some(err) = err.downcast_ref::<anyhow::Error>() {
					log::error!("Error shutting down thread: {err}");
				} else if let Some(err) = err.downcast_ref::<Box<dyn std::error::Error>>() {
					log::error!("Error shutting down thread: {err}");
				} else if let Some(err) = err.downcast_ref::<Box<dyn std::error::Error + Send + Sync + 'static>>() {
					log::error!("Error shutting down thread: {err}");
				} else if let Some(err) = err.downcast_ref::<Box<dyn std::fmt::Display>>() {
					log::error!("Error shutting down thread: {err}");
				} else if let Some(err) = err.downcast_ref::<Box<dyn std::fmt::Debug>>() {
					log::error!("Error shutting down thread: {err:?}");
				} else {
					log::error!("Error shutting down thread: {err:?}");
				}
			}
		}

		ocr::shutdown();

		shutdown_tx.send(()).ok();
	});
}

fn shutdown() -> bool {
	static SHUTDOWN_COUNT: prelude::AtomicU8 = prelude::AtomicU8::new(0);

	if SHUTDOWN_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 2 {
		log::info!("forcing shutdown");
		std::process::exit(1);
	}

	log::info!("shutting down...");

	if !SHUTDOWN.swap(true, std::sync::atomic::Ordering::SeqCst) {
		capture::unpark();

		ui::shutdown();

		true
	} else {
		false
	}
}

fn ctrlc_shutdown() {
	if shutdown() {
		if let Some(rx) = JOIN_MAIN_THREAD.get() {
			rx.recv().ok(); // joins threads
		}
	}
}

fn is_shutdown() -> bool {
	SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed)
}
