use super::*;

const CUSTOM_MARKER_COLOR: [f32; 3] = [1.0, 0.0, 1.0];
const MEASURE_MARKER_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

#[derive(Default)]
pub struct DrawState {
	pub custom_markers: Vec<[[f32; 2]; 2]>,
	measure_start: Option<[f32; 2]>,
	drag_start: Option<[f32; 2]>,
	drag_debounce: bool
}

pub(super) fn delete_marker(state: &mut UIState, i: usize) {
	if i >= state.draw.custom_markers.len() {
		return;
	}

	state.draw.custom_markers.remove(i);

	if let Some(web) = &state.web.server {
		web.send(smh_web::Event::Markers { markers: Box::from(&*state.draw.custom_markers), custom: true });
	}
}

pub(super) fn add_marker(state: &mut UIState, p0: [f32; 2], p1: [f32; 2]) {
	state.draw.custom_markers.push([p0, p1]);

	if let Some(web) = &state.web.server {
		web.send(smh_web::Event::Markers { markers: Box::from(&*state.draw.custom_markers), custom: true });
	}
}

fn is_line_long_enough(ui: &Ui, p0: [f32; 2], p1: [f32; 2]) -> bool {
	p0.into_iter().zip(p1.into_iter()).map(|(a, b)| (a - b).powi(2)).sum::<f32>() >= ui.io().mouse_drag_threshold.powi(2)
}

fn is_point_on_line(a: [f32; 2], b: [f32; 2], c: [f32; 2], tolerance: f32) -> bool {
	let cross_product = (c[1] - a[1]) * (b[0] - a[0]) - (c[0] - a[0]) * (b[1] - a[1]);
	if cross_product.abs() > tolerance {
		return false;
	}

	let dot_product = (c[0] - a[0]) * (b[0] - a[0]) + (c[1] - a[1]) * (b[1] - a[1]);
	if dot_product < 0.0 {
		return false;
	}

	let length = (b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]);
	if dot_product > length {
		return false;
	}

	true
}

fn mouse_ctl(state: &mut UIState, ui: &Ui) {
	let can_drag = ui.is_window_focused() && !ui.is_any_item_hovered() && !ui.is_any_item_focused();
	if !can_drag || ((state.draw.drag_start.is_some() || state.draw.measure_start.is_some()) && ui.is_key_down(imgui::Key::Escape)) {
		state.draw.drag_start = None;
		state.draw.measure_start = None;
		state.draw.drag_debounce = true;
		return;
	}

	if state.draw.drag_debounce {
		if !ui.is_mouse_down(imgui::MouseButton::Left) && !ui.is_mouse_down(imgui::MouseButton::Right) {
			state.draw.drag_debounce = false;
		}
		return;
	}

	if state.draw.drag_start.is_none() {
		if state.draw.measure_start.is_some() {
			if !ui.is_mouse_down(imgui::MouseButton::Right) {
				state.draw.measure_start = None;
				state.draw.drag_debounce = true;
			}
		} else if can_drag && ui.is_mouse_down(imgui::MouseButton::Right) {
			let mouse_pos = ui.io().mouse_pos;
			if mouse_pos != [f32::MAX, f32::MAX] {
				state.draw.measure_start = Some(state.map.viewport.inverse_xy(mouse_pos));
			}
		} else if can_drag && ui.is_mouse_down(imgui::MouseButton::Left) {
			let mouse_pos = ui.io().mouse_pos;
			if mouse_pos != [f32::MAX, f32::MAX] {
				state.draw.drag_start = Some(state.map.viewport.inverse_xy(mouse_pos));
			}
		}
	} else if !ui.is_mouse_down(imgui::MouseButton::Left) {
		let drag_start = state.draw.drag_start.take().unwrap();

		let mouse_pos = ui.io().mouse_pos;
		if mouse_pos != [f32::MAX, f32::MAX] {
			let mouse_pos = state.map.viewport.inverse_xy(mouse_pos);

			let dist = mouse_pos
				.into_iter()
				.zip(drag_start.into_iter())
				.map(|(a, b)| (a - b).powi(2))
				.sum::<f32>();

			if dist >= ui.io().mouse_drag_threshold.powi(2) {
				add_marker(state, drag_start, mouse_pos);
			}
		}
	}
}

fn delete_ctl(state: &mut UIState, ui: &Ui, p0: [f32; 2], p1: [f32; 2], i: usize) -> bool {
	if ui.is_any_item_focused() || ui.is_any_item_hovered() {
		return true;
	}

	let mouse_pos = ui.io().mouse_pos;
	if mouse_pos == [f32::MAX, f32::MAX] {
		return true;
	}

	let tolerance = ((state.display_size[0] * state.display_size[1]).sqrt() / 554.0) * 2000.0;

	if is_point_on_line(p0, p1, mouse_pos, tolerance) {
		if ui.is_mouse_clicked(imgui::MouseButton::Left) || ui.is_mouse_clicked(imgui::MouseButton::Right) {
			delete_marker(state, i);
			state.draw.drag_debounce = true;
			return false;
		} else {
			ui.set_mouse_cursor(Some(imgui::MouseCursor::Hand));
		}
	}

	true
}

pub(super) fn render(state: &mut UIState, ui: &Ui) {
	let mut i = 0;
	while i < state.draw.custom_markers.len() {
		let [p0, p1] = state.draw.custom_markers[i];

		markers::draw(
			state,
			ui,
			&Marker::new(p0, p1, state.vision.meters_to_px_ratio),
			CUSTOM_MARKER_COLOR,
			DrawList::Background
		);

		if delete_ctl(state, ui, state.map.viewport.translate_xy(p0), state.map.viewport.translate_xy(p1), i) {
			i += 1;
		}
	}

	mouse_ctl(state, ui);

	if i < state.draw.custom_markers.len() {
		let [p0, p1] = state.draw.custom_markers[i];

		markers::draw(
			state,
			ui,
			&Marker::new(p0, p1, state.vision.meters_to_px_ratio),
			CUSTOM_MARKER_COLOR,
			DrawList::Background
		);
	}

	if let Some(drag_start) = state.draw.drag_start {
		let mouse_pos = ui.io().mouse_pos;
		if mouse_pos != [f32::MAX, f32::MAX] {
			let mouse_pos = state.map.viewport.inverse_xy(mouse_pos);
			if is_line_long_enough(ui, drag_start, mouse_pos) {
				markers::draw(
					state,
					ui,
					&Marker::new(drag_start, mouse_pos, state.vision.meters_to_px_ratio),
					CUSTOM_MARKER_COLOR,
					DrawList::Background
				);
			}
		}
	}

	if let Some(measure_start) = state.draw.measure_start {
		let mouse_pos = ui.io().mouse_pos;
		if mouse_pos != [f32::MAX, f32::MAX] {
			let mouse_pos = state.map.viewport.inverse_xy(mouse_pos);
			if is_line_long_enough(ui, measure_start, mouse_pos) {
				markers::draw(
					state,
					ui,
					&Marker::new(measure_start, mouse_pos, state.vision.meters_to_px_ratio),
					MEASURE_MARKER_COLOR,
					DrawList::Background
				);
			}
		}
	}
}
