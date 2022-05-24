use super::*;

#[derive(Default)]
pub(super) struct MapState {
	pub viewport: MapViewport,
	texture: Option<(u32, TextureId)>,
	pan_pos: Point<f32>,
	pan_start: Option<Point<f32>>,
	zoom_pos: Point<f32>,
	zoom: u8,
}

#[derive(Default, Debug)]
pub struct MapViewport {
	pub scale_factor_w: f32,
	pub scale_factor_h: f32,
	pub top_left: [f32; 2],
}
impl MapViewport {
	#[inline]
	pub fn calc(
		region_w: f32,
		region_h: f32,
		actual_w: f32,
		actual_h: f32,
		zoom: u8,
		zoom_pos: Point<f32>,
		pan_pos: Point<f32>,
	) -> (Rect<f32>, Self) {
		let map_aspect_ratio = actual_w / actual_h;
		let window_aspect_ratio = region_w / region_h;

		let mut size = if window_aspect_ratio > map_aspect_ratio {
			[region_h * map_aspect_ratio, region_h]
		} else {
			let map_aspect_ratio = actual_h / actual_w;
			[region_w, region_w * map_aspect_ratio]
		};

		// Zoom + pan
		let top_left = {
			let mut top_left = [(region_w - size[0]) / 2., (region_h - size[1]) / 2.];

			if zoom != 0 {
				// 0% to 200%
				let mut zoom_amount = (zoom as f32 / ZOOM_LEVELS as f32).min(1.0) * MAX_ZOOM;

				top_left[0] -= zoom_pos.x * size[0] * zoom_amount;
				top_left[1] -= zoom_pos.y * size[1] * zoom_amount;

				// Pan
				top_left[0] += pan_pos.x * (size[0] / actual_w);
				top_left[1] += pan_pos.y * (size[1] / actual_h);

				zoom_amount += 1.0;

				size[0] *= zoom_amount;
				size[1] *= zoom_amount;
			}

			top_left
		};

		(
			Rect {
				left: top_left[0],
				top: top_left[1],
				right: top_left[0] + size[0],
				bottom: top_left[1] + size[1]
			},
			MapViewport {
				scale_factor_w: size[0] / actual_w,
				scale_factor_h: size[1] / actual_h,
				top_left,
			},
		)
	}

	#[inline]
	pub fn translate_x<T: Copy>(&self, x: T) -> f32
	where
		f32: LossyFrom<T>,
	{
		(f32::lossy_from(x) * self.scale_factor_w) + self.top_left[0]
	}

	#[inline]
	pub fn translate_y<T: Copy>(&self, y: T) -> f32
	where
		f32: LossyFrom<T>,
	{
		(f32::lossy_from(y) * self.scale_factor_h) + self.top_left[1]
	}

	#[inline]
	pub fn translate_xy<T: Copy>(&self, xy: [T; 2]) -> [f32; 2]
	where
		f32: LossyFrom<T>,
	{
		[self.translate_x(xy[0]), self.translate_y(xy[1])]
	}

	#[inline]
	pub fn inverse_x<T: Copy>(&self, x: T) -> f32
	where
		f32: LossyFrom<T>,
	{
		(f32::lossy_from(x) - self.top_left[0]) / self.scale_factor_w
	}

	#[inline]
	pub fn inverse_y<T: Copy>(&self, y: T) -> f32
	where
		f32: LossyFrom<T>,
	{
		(f32::lossy_from(y) - self.top_left[1]) / self.scale_factor_h
	}

	#[inline]
	pub fn inverse_xy<T: Copy>(&self, xy: [T; 2]) -> [f32; 2]
	where
		f32: LossyFrom<T>,
	{
		[self.inverse_x(xy[0]), self.inverse_y(xy[1])]
	}
}

const MAX_ZOOM: f32 = 4.0;
const ZOOM_LEVELS: u8 = 10;
const PAN_ACCELERATION: f32 = 2.0;

pub(super) fn zoom_ctl(state: &mut UIState, ui: &Ui) {
	if ui.is_any_item_focused() || ui.is_any_item_hovered() {
		return;
	}

	let mouse_wheel = ui.io().mouse_wheel;
	if mouse_wheel != 0.0 {
		if mouse_wheel > 0.0 {
			if state.map.zoom < ZOOM_LEVELS {
				let from_zero = state.map.zoom == 0;

				state.map.zoom += 1;

				let mouse_pos = ui.io().mouse_pos;
				let mouse_pos_f = [
					(mouse_pos[0] / state.display_size[0]).min(1.0),
					(mouse_pos[1] / state.display_size[1]).min(1.0),
				];
				if from_zero {
					state.map.zoom_pos = Point::from(mouse_pos_f);
				} else {
					state.map.zoom_pos = (state.map.zoom_pos + Point::from(mouse_pos_f)) / 2.0;
				}
			}
		} else if let Some(map_zoom) = state.map.zoom.checked_sub(1) {
			if map_zoom == 0 {
				state.map.pan_start = None;
				state.map.pan_pos = Point::new(0.0, 0.0);
			}

			state.map.zoom = map_zoom;
		}
	}

	if state.map.zoom != 0 && ui.is_mouse_down(imgui::MouseButton::Middle) {
		let mouse_pos = Point::from(ui.io().mouse_pos);

		if let Some(map_pan_start) = state.map.pan_start {
			let delta = mouse_pos - map_pan_start;
			state.map.pan_pos += delta * PAN_ACCELERATION;
		}

		state.map.pan_start = Some(mouse_pos);
	} else {
		state.map.pan_start = None;
	}
}

fn create_map_texture(state: &mut UIState, map: &image::RgbaImage) -> Result<TextureId, glium::texture::TextureCreationError> {
	let texture = Texture {
		texture: Rc::new(Texture2d::with_format(
			state.display.get_context(),
			RawImage2d {
				width: map.width(),
				height: map.height(),
				data: Cow::Borrowed(map.as_bytes()),
				format: glium::texture::ClientFormat::U8U8U8U8,
			},
			glium::texture::UncompressedFloatFormat::U8U8U8U8,
			glium::texture::MipmapsOption::NoMipmap,
		)?),
		sampler: SamplerBehavior {
			magnify_filter: glium::uniforms::MagnifySamplerFilter::Linear,
			minify_filter: glium::uniforms::MinifySamplerFilter::Linear,
			..Default::default()
		},
	};

	Ok(if let Some((_, texture_id)) = state.map.texture {
		state.renderer.textures().replace(texture_id, texture);
		texture_id
	} else {
		state.renderer.textures().insert(texture)
	})
}

pub(super) fn render(state: &mut UIState, ui: &Ui) {
	let map = state.vision.debug.debug_view.get_image().unwrap_or(&state.vision.map).clone();

	let (map_w, map_h) = (map.width() as f32, map.height() as f32);

	// We'll calculate a CRC32 of the map every time we capture it, and if it's unchanged, we'll skip generating a new texture for it
	if state.new_data {
		let map_crc32 = crc32fast::hash(map.as_bytes());
		if state.map.texture.as_ref().map(|(map_crc32, _)| *map_crc32) != Some(map_crc32) {
			// Update web users
			if let Some(web) = &mut state.web.server {
				web.send(smh_web::Event::Map { map: map.clone() });
				web.send(smh_web::Event::Markers {
					custom: false,
					markers: state.vision.markers.iter().map(|marker| [marker.p0, marker.p1]).collect::<Box<_>>(),
				});
			}

			match create_map_texture(state, &*map) {
				Ok(map_texture) => state.map.texture = Some((map_crc32, map_texture)),
				Err(err) => log::warn!("Failed to create map texture: {err}"),
			}
		}
	}

	let (quad, map_viewport) = MapViewport::calc(
		state.display_size[0],
		state.display_size[1],
		map_w,
		map_h,
		state.map.zoom,
		state.map.zoom_pos,
		state.map.pan_pos,
	);

	state.map.viewport = map_viewport;

	zoom_ctl(state, ui);

	if let Some((_, texture_id)) = state.map.texture {
		ui.get_background_draw_list()
			.add_image_quad(texture_id, quad.top_left(), quad.top_right(), quad.bottom_right(), quad.bottom_left())
			.build();
	}

	heightmaps::render_overlay(state, ui);

	draw::render(state, ui);

	let markers_n = state.vision.markers.len();
	state.vision.markers
		.iter()
		.enumerate()
		.map(|(i, marker)| {
			let f = (i + 1) as f32 / markers_n as f32;
			(marker, [1. - f, f, 0.0])
		})
		.for_each(|(marker, color)| {
			markers::draw(state, ui, marker, color, DrawList::Background);
		});
}
