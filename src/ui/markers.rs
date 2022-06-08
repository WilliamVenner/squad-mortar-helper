use super::*;

#[inline]
fn draw_right_info_text(ui: &Ui, text: &str, x: f32, mut y: f32, color: [f32; 3], dl: &imgui::DrawListMut) {
	let text_size = ui.calc_text_size(text)[0];
	for line in text.split('\n') {
		let line_size = ui.calc_text_size(line);
		dl.add_text([x + (text_size - line_size[0]), y], color, line);
		y += line_size[1];
	}
}

#[inline]
fn draw_centered_info_text(ui: &Ui, text: &str, x: f32, mut y: f32, color: [f32; 3], dl: &imgui::DrawListMut) {
	let text_size = ui.calc_text_size(text)[0];
	for line in text.split('\n') {
		let line_size = ui.calc_text_size(line);
		dl.add_text([x + ((text_size - line_size[0]) / 2.0), y], color, line);
		y += line_size[1];
	}
}

pub(super) fn draw(state: &UIState, ui: &Ui, marker: &Marker, color: [f32; 3], draw_list: DrawList) {
	use std::fmt::Write;

	let dl = draw_list.get(ui);

	let [p0, p1] = [state.map.viewport.translate_xy(marker.p0), state.map.viewport.translate_xy(marker.p1)];

	dl.add_line(p0, p1, color).thickness(2.0).build();

	let mut meters = None;

	let midpoint = Point::new((p0[0] + p1[0]) / 2., (p0[1] + p1[1]) / 2.);

	let mut alt_delta = None;
	if let Some(minimap_viewport) = state.vision.minimap_bounds {
		if let Some(heightmap) = squadex::heightmaps::get_current() {
			let offset = if state.heightmaps.use_heightmap_offset {
				let offset = [heightmap.bounds[0][0] as f32, heightmap.bounds[0][1] as f32];

				let hm_scale_factor_w = minimap_viewport.width() as f32 / (heightmap.width as f32 + offset[0]);
				let hm_scale_factor_h = minimap_viewport.height() as f32 / (heightmap.height as f32 + offset[1]);

				[
					offset[0] * hm_scale_factor_w * state.map.viewport.scale_factor_w,
					offset[1] * hm_scale_factor_h * state.map.viewport.scale_factor_h,
				]
			} else {
				[0.0, 0.0]
			};

			let minimap_viewport = Rect {
				left: state.map.viewport.translate_x(minimap_viewport.left as f32) + offset[0],
				top: state.map.viewport.translate_y(minimap_viewport.top as f32) + offset[1],
				right: state.map.viewport.translate_x(minimap_viewport.right as f32),
				bottom: state.map.viewport.translate_y(minimap_viewport.bottom as f32),
			};

			let p0_xf = (p0[0] as f64 - minimap_viewport.left as f64) / minimap_viewport.width() as f64;
			let p0_yf = (p0[1] as f64 - minimap_viewport.top as f64) / minimap_viewport.height() as f64;

			let p1_xf = (p1[0] as f64 - minimap_viewport.left as f64) / minimap_viewport.width() as f64;
			let p1_yf = (p1[1] as f64 - minimap_viewport.top as f64) / minimap_viewport.height() as f64;

			let p0_x = p0_xf * heightmap.width as f64;
			let p0_y = p0_yf * heightmap.height as f64;
			let p1_x = p1_xf * heightmap.width as f64;
			let p1_y = p1_yf * heightmap.height as f64;

			// The heightmap can be used to calculate a more accurate length than eyeballing the map scales
			meters = Some(((p0_x - p1_x).powi(2) + (p0_y - p1_y).powi(2)).sqrt());

			let p0_x = p0_x.round() as i32;
			let p0_y = p0_y.round() as i32;
			let p1_x = p1_x.round() as i32;
			let p1_y = p1_y.round() as i32;

			if p0_x >= 0
				&& p0_y >= 0 && p1_x >= 0
				&& p1_y >= 0 && p0_x < heightmap.width as i32
				&& p0_y < heightmap.height as i32
				&& p1_x < heightmap.width as i32
				&& p1_y < heightmap.height as i32
			{
				alt_delta = Some((heightmap.height(p1_x as _, p1_y as _) as i32 - heightmap.height(p0_x as _, p0_y as _) as i32) as f64);
			} else {
				meters = None;
			}
		}
	}

	let meters = match meters.or(marker.meters) {
		Some(meters) => meters,
		None => return,
	};

	let angle = f32::atan2(p0[1] - p1[1], p0[0] - p1[0]);

	let mut bearing_fwd = angle.to_degrees();
	if bearing_fwd > 0.0 {
		bearing_fwd -= 90.0;
		if bearing_fwd < 0.0 {
			bearing_fwd += 360.0;
		}
	} else {
		bearing_fwd += 270.0;
	}
	bearing_fwd = bearing_fwd.round() % 360.0;
	let bearing_bck = (bearing_fwd + 180.0).round() % 360.0;

	let text_angle = if angle >= core::f32::consts::FRAC_PI_2 {
		angle - core::f32::consts::PI
	} else if angle <= -core::f32::consts::FRAC_PI_2 {
		angle + core::f32::consts::PI
	} else {
		angle
	};

	let rotate = ui.rotate(text_angle, Some(midpoint.into()), draw_list);
	let font = ui.push_font(state.fonts.marker_label);

	if let Some(alt_delta_fwd) = alt_delta {
		let alt_delta_bck = alt_delta_fwd * -1.0;

		let meters_text = bumpalo::format!(in &state.ui_fmt_alloc, "{:.0}m\n±{}m alt", meters, (alt_delta_fwd as i32).abs() as u32);
		let meters_text_size = ui.calc_text_size(&meters_text);
		let meters_text_pos = [midpoint.x - (meters_text_size[0] / 2.0), midpoint.y];
		draw_centered_info_text(ui, &meters_text, meters_text_pos[0], meters_text_pos[1], color, &dl);

		let flip = (-core::f32::consts::FRAC_PI_2..core::f32::consts::FRAC_PI_2).contains(&angle);
		let fwd = {
			let (alt_delta, bearing) = if flip {
				(alt_delta_fwd, bearing_fwd)
			} else {
				(alt_delta_bck, bearing_bck)
			};

			let mut info = bumpalo::collections::String::with_capacity_in(256, &state.ui_fmt_alloc);

			let milliradians = squadex::milliradians::calc(meters, alt_delta);
			if milliradians.is_nan() {
				info.push_str("<- RANGE!");
			} else {
				write!(info, "<- {:.0} mil", milliradians).unwrap();
			}

			write!(info, "\n{:.0}°", bearing).unwrap();

			#[cfg(debug_assertions)]
			write!(info, "\n{}m alt", alt_delta as i32).unwrap();

			info
		};
		let bck = {
			let (alt_delta, bearing) = if flip {
				(alt_delta_bck, bearing_bck)
			} else {
				(alt_delta_fwd, bearing_fwd)
			};

			let mut info = bumpalo::collections::String::with_capacity_in(256, &state.ui_fmt_alloc);

			let milliradians = squadex::milliradians::calc(meters, alt_delta);
			if milliradians.is_nan() {
				info.push_str("RANGE! ->");
			} else {
				write!(info, "{:.0} mil ->", milliradians).unwrap();
			}

			write!(info, "\n{:.0}°", bearing).unwrap();

			#[cfg(debug_assertions)]
			write!(info, "\n{}m alt", alt_delta as i32).unwrap();

			info
		};

		let fwd_text_size = ui.calc_text_size(&fwd)[0];
		let bck_text_size = ui.calc_text_size(&bck)[0];
		let text_width = fwd_text_size + bck_text_size;

		let fwd_text_pos = midpoint.x - ((text_width + 10.0) / 2.0);
		let bck_text_pos = fwd_text_pos + 10.0 + fwd_text_size;

		draw_right_info_text(ui, &fwd, fwd_text_pos, midpoint.y + meters_text_size[1], color, &dl);
		dl.add_text([bck_text_pos, midpoint.y + meters_text_size[1]], color, &bck);
	} else {
		let mut info = bumpalo::collections::String::with_capacity_in(256, &state.ui_fmt_alloc);
		writeln!(info, "{:.0}m", meters).unwrap();

		let milliradians = squadex::milliradians::calc(meters, 0.0);
		if milliradians.is_nan() {
			info.push_str("RANGE!");
		} else {
			write!(info, "{:.0} mil", milliradians).unwrap();
		}

		let text_size = Point::from(ui.calc_text_size(&info));
		let text_pos = Point::new(midpoint.x - (text_size.x / 2.0), midpoint.y);

		if (-core::f32::consts::FRAC_PI_2..=core::f32::consts::FRAC_PI_2).contains(&angle) {
			write!(info, "\n-> {:.0}°", bearing_bck).unwrap();
			write!(info, "\n<- {:.0}°", bearing_fwd).unwrap();
		} else {
			write!(info, "\n-> {:.0}°", bearing_fwd).unwrap();
			write!(info, "\n<- {:.0}°", bearing_bck).unwrap();
		}

		draw_centered_info_text(ui, &info, text_pos.x, text_pos.y, color, &dl);
	}

	font.pop();
	rotate.end();
}
