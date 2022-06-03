use super::*;

pub(super) fn draw(state: &UIState, ui: &Ui, marker: &Marker, color: [f32; 3], draw_list: DrawList) {
	use std::fmt::Write;

	let dl = draw_list.get(ui);

	let [p0, p1] = [state.map.viewport.translate_xy(marker.p0), state.map.viewport.translate_xy(marker.p1)];

	dl.add_line(p0, p1, color).thickness(2.0).build();

	let mut meters = None;

	let midpoint = Point::new(
		(p0[0] + p1[0]) / 2.,
		(p0[1] + p1[1]) / 2.
	);

	let mut alt_delta = None;
	if let Some(minimap_viewport) = state.vision.minimap_bounds {
		if let Some(heightmap) = squadex::heightmaps::get_current() {
			let offset = [heightmap.bounds[0][0] as f32, heightmap.bounds[0][1] as f32];

			let hm_scale_factor_w = minimap_viewport.width() as f32 / (heightmap.width as f32 + offset[0]);
			let hm_scale_factor_h = minimap_viewport.height() as f32 / (heightmap.height as f32 + offset[1]);
			let offset = [offset[0] * hm_scale_factor_w * state.map.viewport.scale_factor_w, offset[1] * hm_scale_factor_h * state.map.viewport.scale_factor_h];

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

			if p0_x >= 0 && p0_y >= 0 && p1_x >= 0 && p1_y >= 0 && p0_x < heightmap.width as i32 && p0_y < heightmap.height as i32 && p1_x < heightmap.width as i32 && p1_y < heightmap.height as i32 {
				let p0 = heightmap.data[p0_y as usize * heightmap.width as usize + p0_x as usize];
				let p1 = heightmap.data[p1_y as usize * heightmap.width as usize + p1_x as usize];

				alt_delta = Some((p0.abs_diff(p1) as f32 / heightmap.scale[2]).round() as u16);
			}
		}
	}

	let meters = match meters.or(marker.meters) {
		Some(meters) => meters,
		None => return
	};

	let mut info = String::with_capacity(256);
	writeln!(info, "{:.0}m", meters).unwrap();

	let milliradians = squadex::milliradians::calc(meters, alt_delta.map(|d| d as f64).unwrap_or_default());
	if milliradians.is_nan() {
		info.push_str("RANGE!");
	} else {
		write!(info, "{:.0} mils", milliradians).unwrap();
	}

	if let Some(alt_delta) = alt_delta {
		write!(info, "\n{}m alt", alt_delta).unwrap();
	}

	let font = ui.push_font(state.fonts.marker_label);

	let text_size = Point::from(ui.calc_text_size(&info));
	let text_pos = Point::new(midpoint.x - (text_size.x / 2.0), midpoint.y);

	let mut angle = f32::atan2(p0[1] - p1[1], p0[0] - p1[0]);

	let mut bearing = angle.to_degrees();
	if bearing > 0.0 {
		bearing -= 90.0;
		if bearing < 0.0 {
			bearing += 360.0;
		}
	} else {
		bearing += 270.0;
	}
	let bearing_bck = (bearing + 180.0) % 360.0;

	if (-core::f32::consts::FRAC_PI_2..=core::f32::consts::FRAC_PI_2).contains(&angle) {
		write!(info, "\n-> {:.0}°", bearing_bck).unwrap();
		write!(info, "\n<- {:.0}°", bearing).unwrap();
	} else {
		write!(info, "\n-> {:.0}°", bearing).unwrap();
		write!(info, "\n<- {:.0}°", bearing_bck).unwrap();
	}

	if angle >= core::f32::consts::FRAC_PI_2 {
		angle -= core::f32::consts::PI;
	} else if angle <= -core::f32::consts::FRAC_PI_2 {
		angle += core::f32::consts::PI;
	}

	let rotate = ui.rotate(angle, Some(midpoint.into()), draw_list);
	dl.add_text(text_pos.into(), color, &info);
	rotate.end();

	font.pop();
}