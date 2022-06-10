use super::*;

fn set_window_icon(window: &winit::window::Window) -> Result<(), AnyError> {
	let ico = image::load_from_memory_with_format(include_bytes!("../../mortar.ico"), image::ImageFormat::Ico)?.into_rgba8();
	let ico = image::imageops::resize(&ico, 64, 64, image::imageops::FilterType::Nearest);
	let ico = ico.into_vec();
	window.set_window_icon(Some(winit::window::Icon::from_rgba(ico, 64, 64)?));
	Ok(())
}

pub fn start<F: FnOnce() + 'static>(vision_thread: std::thread::Thread, logs: logs::LogState, shutdown: F) -> ! {
	let mut shutdown = Some(shutdown);

	let event_loop = EventLoop::<UIEvent>::with_user_event();

	unsafe { TX.set(event_loop.create_proxy()) };

	let context = glutin::ContextBuilder::new()
		.with_vsync(true);

	let builder = WindowBuilder::new()
		.with_resizable(true)
		.with_title(concat!("Squad Mortar Helper v", env!("CARGO_PKG_VERSION"), " by Billy"))
		.with_transparent(true);

	let display = Display::new(builder, context, &event_loop).expect("Failed to initialize display");

	let mut imgui: Context = Context::create();
	imgui.set_ini_filename(None);

	match clipboard::init() {
		Ok(clipboard) => imgui.set_clipboard_backend(clipboard),
		Err(err) => log::error!("Failed to initialize clipboard backend: {err}")
	}

	let mut platform = WinitPlatform::init(&mut imgui);
	{
		let gl_window = display.gl_window();
		let window = gl_window.window();

		let _icon_res = set_window_icon(window);
		debug_assert!(_icon_res.is_ok(), "{_icon_res:?}");

		platform.attach_window(imgui.io_mut(), window, HiDpiMode::Default);
	};

	super::DPI_ESTIMATE.store((96.0 * platform.hidpi_factor()).round() as u32, std::sync::atomic::Ordering::Release);

	let fonts = fonts::add(&mut imgui, platform.hidpi_factor());

	let renderer = Box::leak(Box::new(Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer")));

	let mut state = UiState::new(display.clone(), renderer, fonts, logs, vision_thread);

	let mut ui_data_update_id = usize::MAX;
	let mut redraw_amount: u8 = 1;

	theme::apply();

	let mut last_frame = Instant::now();
	event_loop.run(move |event, _, control_flow| match event {
		Event::UserEvent(UIEvent::Shutdown) | Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
			*control_flow = ControlFlow::Exit;
		}

		Event::LoopDestroyed => {
			crate::shutdown();

			if let Some(shutdown) = shutdown.take() {
				shutdown();
			}

			log::info!("ui shutting down...");
		}

		Event::NewEvents(_) => {
			let now = Instant::now();
			imgui.io_mut().update_delta_time(now - last_frame);
			last_frame = now;
		}

		Event::RedrawRequested(_) => for _ in 0..core::mem::replace(&mut redraw_amount, 1) {
			state.frame = state.frame.wrapping_add(1);

			let ui_data = UI_DATA.lock();
			state.new_data = core::mem::replace(&mut ui_data_update_id, ui_data.0) != ui_data.0 && !ui_data.1.sleeping;
			state.display_size = imgui.io().display_size;
			state.map.viewport = Default::default();

			let ui_data = parking_lot::MutexGuard::map(ui_data, |ui_data| &mut ui_data.1);

			let gl_window = display.gl_window();
			platform.prepare_frame(imgui.io_mut(), gl_window.window()).expect("Failed to prepare frame");

			let ui = imgui.frame();
			state.vision.set(ui_data);
			state.render(&ui);
			state.vision.take();

			let mut target = display.draw();

			target.clear_color_srgb(0.0, 0.0, 0.0, 0.0);

			platform.prepare_render(&ui, gl_window.window());

			let draw_data = ui.render();

			state.renderer
				.render(&mut target, draw_data)
				.expect("Rendering failed");

			target.finish().expect("Failed to swap buffers");
		}

		event @ Event::WindowEvent { event: WindowEvent::MouseInput { .. } | WindowEvent::KeyboardInput { .. } | WindowEvent::MouseWheel { .. }, .. } => {
			// Draw two frames instead of just the one as the UI doesn't update properly otherwise
			redraw_amount = 2;

			let gl_window = display.gl_window();
			let window = gl_window.window();
			platform.handle_event(imgui.io_mut(), window, &event);
			window.request_redraw();
		},

		event @ (
			Event::UserEvent(UIEvent::Redraw) |
			Event::Resumed |
			Event::WindowEvent {
				event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } | WindowEvent::CursorMoved { .. },
				..
			}
		) => {
			let gl_window = display.gl_window();
			let window = gl_window.window();
			platform.handle_event(imgui.io_mut(), window, &event);
			window.request_redraw();
		},

		event => {
			platform.handle_event(imgui.io_mut(), display.gl_window().window(), &event);
		}
	})
}