use super::*;

pub(super) fn menu_bar(state: &UiState, ui: &Ui) {
	let paused = SETTINGS.paused();
	let mut toggle_pause = ui.is_key_pressed(imgui::Key::Space);

	if let Some(settings) = ui.begin_menu("Settings") {
		if imgui::MenuItem::new("PAUSE").selected(paused).shortcut("Space").build(ui) {
			toggle_pause = true;
		}

		#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
		{
			let hardware_acceleration = SETTINGS.hardware_acceleration();
			if imgui::MenuItem::new("Hardware Acceleration (GPU)")
				.selected(hardware_acceleration)
				.build(ui)
			{
				SETTINGS.set_hardware_acceleration(!hardware_acceleration);
			}
		}

		let detect_markers = SETTINGS.detect_markers();
		if imgui::MenuItem::new("Detect Markers").selected(detect_markers).build(ui) {
			SETTINGS.set_detect_markers(!detect_markers);
		}

		let grayscale_map = SETTINGS.grayscale_map();
		if imgui::MenuItem::new("Grayscale Map").selected(grayscale_map).build(ui) {
			SETTINGS.set_grayscale_map(!grayscale_map);
		}

		settings.end();
	}

	if toggle_pause {
		SETTINGS.set_paused(!paused, &state.vision_thread);
	}
}

pub(super) fn render_paused_overlay(state: &UiState, ui: &Ui) {
	if !SETTINGS.paused() {
		return;
	};

	ui.set_cursor_screen_pos([0.0, 0.0]);

	let window = {
		let bg = ui.push_style_color(imgui::StyleColor::ChildBg, [0.0, 0.0, 0.0, 1.0]);
		let padding = ui.push_style_var(imgui::StyleVar::WindowPadding([0.0, 0.0]));
		let border = ui.push_style_var(imgui::StyleVar::WindowBorderSize(0.0));

		let window = imgui::ChildWindow::new("PausedOverlay")
			.bg_alpha(0.5)
			.scroll_bar(false)
			.scrollable(false)
			.no_inputs()
			.no_nav()
			.focused(false)
			.focus_on_appearing(false)
			.movable(false)
			.begin(ui);

		padding.end();
		border.end();
		bg.end();
		window
	};
	let window = match window {
		Some(window) => window,
		None => return,
	};

	let text = "PAUSED\nPress SPACE to unpause";
	let font = ui.push_font(state.fonts.paused);

	ui.set_cursor_screen_pos([
		(state.display_size[0] - ui.calc_text_size(&text[.."PAUSED\n".len()])[0]) / 2.0,
		(state.display_size[1] - ui.calc_text_size(text)[1]) / 2.0,
	]);
	ui.text(&text[0.."PAUSED\n".len()]);

	ui.set_cursor_screen_pos([
		(state.display_size[0] - ui.calc_text_size(&text["PAUSED\n".len()..])[0]) / 2.0,
		ui.cursor_screen_pos()[1],
	]);
	ui.text(&text["PAUSED\n".len()..]);

	font.pop();

	window.end();
}
