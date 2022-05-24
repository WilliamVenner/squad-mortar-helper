use super::*;
use smh_vision_common::debug::*;

#[derive(Default)]
pub(super) struct DebugState {
	fps: bool,
	ocr_overlay: bool,
	scales_overlay: bool,
	minimap_bounds_overlay: bool,
}

pub(super) fn menu_bar(ui: &Ui, state: &mut UIState) {
	let debug = match ui.begin_menu("Debug") {
		Some(debug) => debug,
		None => return,
	};

	if imgui::MenuItem::new("Logs").selected(state.logs.window_open).build(ui) {
		state.logs.window_open = !state.logs.window_open;
	}

	if imgui::MenuItem::new("Show FPS").selected(state.debug.fps).build(ui) {
		state.debug.fps = !state.debug.fps;
	}

	let show_ocr = imgui::MenuItem::new("Show OCR").selected(state.debug.ocr_overlay).build(ui);
	let show_scales = imgui::MenuItem::new("Show Computed Scales")
		.selected(state.debug.scales_overlay)
		.build(ui);

	if show_ocr || show_scales {
		if show_ocr {
			state.debug.ocr_overlay = !state.debug.ocr_overlay;
		}
		if show_scales {
			state.debug.scales_overlay = !state.debug.scales_overlay;
		}
		if !state.debug.ocr_overlay && !state.debug.scales_overlay {
			crate::debug::off();
		} else {
			crate::debug::on();
		}
	}

	if imgui::MenuItem::new("Show Minimap Bounds")
		.selected(state.debug.minimap_bounds_overlay)
		.build(ui)
	{
		state.debug.minimap_bounds_overlay = !state.debug.minimap_bounds_overlay;
	}

	if let Some(cv_inputs) = ui.begin_menu("Computer Vision Inputs") {
		let choice = DebugView::get();

		if imgui::MenuItem::new("Map").selected(choice == DebugView::None).build(ui) {
			DebugView::set(DebugView::None);
		}

		for (name, variant) in DebugView::variants() {
			if imgui::MenuItem::new(name).selected(variant == choice).build(ui) {
				DebugView::set(variant);
			}
		}

		cv_inputs.end();
	}

	debug.end();
}

fn draw_fps(state: &mut UIState, ui: &Ui, entire_frame: Duration) {
	use std::fmt::Write;

	let fps = imgui::Window::new("FPS")
		.position([0.0, 20.0], imgui::Condition::Once)
		.size(state.display_size, imgui::Condition::Always)
		.no_decoration()
		.title_bar(false)
		.scroll_bar(false)
		.movable(false)
		.resizable(false)
		.draw_background(false)
		.mouse_inputs(false)
		.begin(ui)
		.unwrap();

	#[inline]
	fn write_fps(str: &mut String, fps: f64) {
		if fps.is_nan() || fps.is_infinite() {
			str.push('0');
		} else if fps < 1.0 {
			write!(str, "{:.2}", fps).ok();
		} else {
			write!(str, "{}", fps.round()).ok();
		}
	}

	let mut bar_w;
	{
		let mut fps = String::with_capacity(64);
		writeln!(&mut fps, "Frame: {}", state.frame).ok();
		fps.push_str("Vision: ");
		write_fps(&mut fps, 1.0 / entire_frame.as_secs_f64());
		fps.push('/');
		writeln!(fps, "{} FPS", vision::FPS).ok();
		writeln!(fps, "Total: {:?}", entire_frame).ok();

		bar_w = ui.calc_text_size(&fps)[0] as f32;
		ui.text(fps);
	}

	{
		let draw_list = ui.get_window_draw_list();

		let y_spacing = ui.text_line_height_with_spacing() - ui.text_line_height();

		let mut total = Duration::ZERO;
		for (name, color, duration) in state.vision.debug.timeshares.iter() {
			total += duration;

			draw_list
				.add_rect(relative![ui, 0.0, y_spacing], relative![ui, 10.0, y_spacing + 10.0], *color)
				.filled(true)
				.build();
			draw_list
				.add_rect(relative![ui, 0.0, y_spacing], relative![ui, 10.0, y_spacing + 10.0], [0.0, 0.0, 0.0])
				.filled(false)
				.thickness(2.0)
				.build();

			let text = ui_format!(state, "{}: {:?}", name, duration);
			bar_w = bar_w.max(ui.calc_text_size(&text)[0] as f32 + 15.0);

			ui.set_cursor_screen_pos(relative![ui, 15.0, 0.0]);
			ui.text(&text);
		}

		if !total.is_zero() {
			let mut x = 0.;
			for (_, color, duration) in state.vision.debug.timeshares.iter() {
				let w = ((duration.as_secs_f64() / total.as_secs_f64()) * (bar_w as f64)) as f32;
				draw_list
					.add_rect(relative![ui, x, y_spacing], relative![ui, x + w, y_spacing + 10.0], *color)
					.filled(true)
					.build();
				x += w;
			}

			draw_list
				.add_rect(relative![ui, 0.0, y_spacing], relative![ui, bar_w, y_spacing + 10.0], [0.0, 0.0, 0.0])
				.filled(false)
				.thickness(2.0)
				.build();

			ui.set_cursor_screen_pos(relative![ui, 0.0, y_spacing + 10.0 + y_spacing]);
		}
	}

	fps.end();
}

pub(super) fn render(state: &mut UIState, ui: &Ui) {
	if state.debug.fps {
		if let Some(entire_frame) = state.vision.debug.timeshares.entire_frame {
			draw_fps(state, ui, entire_frame);
		}
	}

	let font = ui.push_font(state.fonts.ocr_label);

	if state.debug.ocr_overlay {
		let draw_list = ui.get_foreground_draw_list();

		for ocr in state.vision.debug.ocr.iter() {
			let f = ocr.confidence as f32 / 100.0;
			let color = [1.0 - f, f, 0.0];

			let p0 = state.map.viewport.translate_xy([ocr.left as f32, ocr.top as f32]);
			let p1 = state.map.viewport.translate_xy([ocr.right as f32, ocr.bottom as f32]);

			draw_list.add_rect(p0, p1, color).build();

			let text_pos = state.map.viewport.translate_xy([ocr.left as f32, ocr.bottom as f32]);
			draw_list.add_text(text_pos, color, ui_format!(state, "{:.2}%\n{:?}", ocr.confidence, ocr.text));
		}
	}

	if state.debug.scales_overlay {
		let draw_list = ui.get_foreground_draw_list();

		for (meters, scale) in state.vision.debug.scales.iter() {
			let [p0, p1] = [
				state.map.viewport.translate_xy(scale.p0.into()),
				state.map.viewport.translate_xy(scale.p1.into()),
			];

			draw_list.add_line(p0, p1, [1.0, 0.0, 1.0]).thickness(2.0).build();
			draw_list.add_text(p0, [1.0, 0.0, 1.0], ui_format!(state, "{}m", meters));
		}
	}

	if state.debug.minimap_bounds_overlay {
		if let Some(minimap_viewport) = state.vision.minimap_bounds {
			let [mut p0, mut p1] = [
				state.map.viewport.translate_xy(minimap_viewport.top_left().map(f32::lossy_from)),
				state.map.viewport.translate_xy(minimap_viewport.bottom_right().map(f32::lossy_from)),
			];

			// seems to be drawn 1 pixel off
			[&mut p0, &mut p1].into_iter().flatten().for_each(|p| {
				*p += 1.0;
			});

			ui.get_foreground_draw_list()
				.add_rect(p0, p1, [0.0, 1.0, 0.0, 1.0])
				.thickness(1.0)
				.build();
		} else {
			let font = ui.push_font(state.fonts.ocr_label);
			let color = ui.push_style_color(imgui::StyleColor::Text, [1.0, 0.0, 0.0, 1.0]);
			let h = ui.calc_text_size("No minimap bounds detected")[1];
			ui.set_cursor_pos([10.0, state.display_size[1] - 10.0 - h]);
			ui.text("No minimap bounds detected");
			font.end();
			color.end();
		}
	}

	font.end();
}
