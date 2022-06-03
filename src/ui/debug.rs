#![allow(clippy::type_complexity)]

use super::*;
use smh_vision_common::{debug::*, markers::{Fireteam, debug_is_map_marker_color}};

pub static SYNCED_DEBUG_STATE: SyncedDebugState = SyncedDebugState {
	debug_view: AtomicU8::new(DebugView::None as u8),
	ocr_overlay: AtomicBool::new(false),
	scales_overlay: AtomicBool::new(false),
};
pub struct SyncedDebugState {
	pub debug_view: AtomicU8,
	pub ocr_overlay: AtomicBool,
	pub scales_overlay: AtomicBool,
}
impl SyncedDebugState {
	pub fn debug_view(&self) -> DebugView {
		DebugView::try_from(self.debug_view.load(std::sync::atomic::Ordering::Acquire)).sus_unwrap()
	}

	pub fn set_debug_view(&self, debug_view: DebugView) {
		self.debug_view.store(debug_view as u8, std::sync::atomic::Ordering::Release);
	}

	pub fn ocr_overlay(&self) -> bool {
		self.ocr_overlay.load(std::sync::atomic::Ordering::Acquire)
	}

	pub fn set_ocr_overlay(&self, ocr_overlay: bool) {
		self.ocr_overlay.store(ocr_overlay, std::sync::atomic::Ordering::Release);
	}

	pub fn scales_overlay(&self) -> bool {
		self.scales_overlay.load(std::sync::atomic::Ordering::Acquire)
	}

	pub fn set_scales_overlay(&self, scales_overlay: bool) {
		self.scales_overlay.store(scales_overlay, std::sync::atomic::Ordering::Release);
	}
}

lazy_static! {
	static ref FAKE_INPUT_SELECTION: Mutex<Option<image::ImageBuffer<image::Bgra<u8>, Box<[u8]>>>> = Mutex::new(None);
}

pub struct FakeInputs {
	selected: Option<Rc<Path>>,
	choices: Box<[(Rc<Path>, Box<str>)]>,
}
impl FakeInputs {
	#[inline]
	pub fn selected() -> Option<image::ImageBuffer<image::Bgra<u8>, Box<[u8]>>> {
		FAKE_INPUT_SELECTION.lock().as_ref().cloned()
	}
}
impl Default for FakeInputs {
	fn default() -> Self {
		Self {
			selected: None,
			choices: {
				let mut choices = std::fs::read_dir("vision-common/samples")
					.map(|choices| {
						choices
							.filter_map(|entry| entry.ok())
							.filter_map(|entry| Some((entry.file_type().ok()?, entry)))
							.filter_map(|entry| {
								if entry.0.is_file() {
									Some((Rc::from(entry.1.path()), entry.1.path().file_name()?.to_string_lossy().into_owned().into_boxed_str()))
								} else {
									None
								}
							})
							.collect::<Box<[_]>>()
					})
					.unwrap_or_default();

				choices.sort_unstable_by(|a, b| a.1.deref().cmp(&*b.1));
				choices
			},
		}
	}
}

#[derive(Default)]
pub struct DebugState {
	vision_debugger: bool,
	minimap_bounds_overlay: bool,

	fps: bool,
	fps_bar_w: f32,

	pub fake_inputs: FakeInputs,
}

#[derive(Debug, Default)]
pub struct DebugBox {
	pub dpi: Option<u32>,
	pub timeshares: Timeshares,
	pub ocr: Vec<smh_vision_ocr::OCRText>,
	pub scales: SmallVec<(u32, Line<u32>), 3>,
	pub debug_view: Option<Arc<image::RgbaImage>>,
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

	let ocr_overlay = SYNCED_DEBUG_STATE.ocr_overlay();
	if imgui::MenuItem::new("Show OCR").selected(ocr_overlay).build(ui) {
		SYNCED_DEBUG_STATE.set_ocr_overlay(!ocr_overlay);
	}

	let scales_overlay = SYNCED_DEBUG_STATE.scales_overlay();
	if imgui::MenuItem::new("Show Computed Scales").selected(scales_overlay).build(ui) {
		SYNCED_DEBUG_STATE.set_scales_overlay(!scales_overlay);
	}

	if imgui::MenuItem::new("Show Minimap Bounds")
		.selected(state.debug.minimap_bounds_overlay)
		.build(ui)
	{
		state.debug.minimap_bounds_overlay = !state.debug.minimap_bounds_overlay;
	}

	if imgui::MenuItem::new("Vision Debugger").selected(state.debug.vision_debugger).build(ui) {
		state.debug.vision_debugger = !state.debug.vision_debugger;
	}

	if let Some(cv_inputs) = ui.begin_menu("Computer Vision Outputs") {
		let debug_view = SYNCED_DEBUG_STATE.debug_view();

		if imgui::MenuItem::new("Map").selected(debug_view == DebugView::None).build(ui) {
			SYNCED_DEBUG_STATE.set_debug_view(DebugView::None);
		}

		for (name, variant) in DebugView::variants() {
			if imgui::MenuItem::new(name).selected(variant == debug_view).build(ui) {
				SYNCED_DEBUG_STATE.set_debug_view(variant);
			}
		}

		cv_inputs.end();
	}

	if !state.debug.fake_inputs.choices.is_empty() {
		if let Some(cv_inputs) = ui.begin_menu("Fake Input") {
			let selected = &mut state.debug.fake_inputs.selected;
			if imgui::MenuItem::new("None").selected(selected.is_none()).build(ui) {
				*FAKE_INPUT_SELECTION.lock() = None;
				*selected = None;
			}

			for choice in state.debug.fake_inputs.choices.iter() {
				if imgui::MenuItem::new(&choice.1)
					.selected(selected.as_ref().map(|selected| Rc::ptr_eq(selected, &choice.0)).unwrap_or_default())
					.build(ui)
				{
					log::info!("Reading fake input image into memory: {}", choice.0.display());

					if let Some(image) = std::fs::read(&*choice.0).ok().and_then(|image| image::load_from_memory(&image).ok()).map(|image| image.into_bgra8()) {
						let image = image::ImageBuffer::from_raw(image.width(), image.height(), image.into_raw().into_boxed_slice()).unwrap();
						*FAKE_INPUT_SELECTION.lock() = Some(image);
						*selected = Some(choice.0.clone());
					}
				}
			}

			cv_inputs.end();
		}
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

		state.debug.fps_bar_w = bar_w.max(state.debug.fps_bar_w).min(state.display_size[0] - (ui.window_padding()[0] * 2.0));
		bar_w = state.debug.fps_bar_w;

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

	if SYNCED_DEBUG_STATE.ocr_overlay() {
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

	if SYNCED_DEBUG_STATE.scales_overlay() {
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

pub(super) fn render_vision_debugger(state: &mut UIState, ui: &Ui) {
	use image::Pixel;

	if !state.debug.vision_debugger {
		return;
	};

	if state.vision.map.is_empty() {
		return;
	};

	let mut abs_mouse_pos = ui.io().mouse_pos;
	if abs_mouse_pos == [f32::MAX, f32::MAX] {
		return;
	};

	let mouse_pos = state.map.viewport.inverse_xy(abs_mouse_pos);
	if mouse_pos.into_iter().any(|xy| xy < 0.0) {
		return;
	};

	let rgb = match state.vision.map.get_pixel_checked(mouse_pos[0] as _, mouse_pos[1] as _) {
		Some(rgb) => rgb.to_rgb(),
		None => return,
	};
	let hsv = rgb.to_hsv();
	let luma8 = rgb.to_luma().0[0];

	let ocr_monochromaticy = smh_vision_cpu::ocr_monochromaticy(rgb);
	let ocr_pixel_brightness = smh_vision_cpu::ocr_brightness(rgb);

	let [alpha, bravo, charlie] = [
		debug_is_map_marker_color(hsv.0, hsv.1, hsv.2, Fireteam::Alpha),
		debug_is_map_marker_color(hsv.0, hsv.1, hsv.2, Fireteam::Bravo),
		debug_is_map_marker_color(hsv.0, hsv.1, hsv.2, Fireteam::Charlie)
	];

	let font = ui.push_font(state.fonts.debug_small);
	let text = ui_format!(
		state,
		"RGB [{}, {}, {}]\nHSV [{}, {}, {}]\nLuma8 {}\nOCRPixelSimilarity {}\nOCRBrightness {}\nAlphaMarker {:?}\nBravoMarker {:?}\nCharlieMarker {:?}",
		rgb.0[0],
		rgb.0[1],
		rgb.0[2],
		hsv.0,
		hsv.1,
		hsv.2,
		luma8,
		ocr_monochromaticy,
		ocr_pixel_brightness,
		alpha,
		bravo,
		charlie
	);

	let window_padding = ui.window_padding();

	let window_size = 200.0;
	let window_size = [
		window_size,
		ui.calc_text_size_with_opts(&text, false, window_size - (window_padding[0] * 2.0))[1] + 10.0 + ui.text_line_height() + window_padding[1],
	];

	font.end();

	let (window_pos, p0, p1) = {
		let mut window_pos = [abs_mouse_pos[0] + 15.0, abs_mouse_pos[1] + 15.0];

		if window_pos[0] + window_size[0] > state.display_size[0] || window_pos[1] + window_size[1] > state.display_size[1] {
			window_pos = [abs_mouse_pos[0] - window_size[0] - 5.0, abs_mouse_pos[1] - window_size[1] - 5.0];
		}

		let p0: [f32; 2] = [window_pos[0] + window_padding[0], window_pos[1] + window_padding[1]];
		let p1: [f32; 2] = (Point::from(p0) + Point::from([window_size[0] - (window_padding[0] * 2.0), 10.0])).into();

		(window_pos, p0, p1)
	};

	let window = imgui::Window::new("Vision Debugger")
		.scroll_bar(false)
		.scrollable(false)
		.movable(false)
		.resizable(false)
		.always_auto_resize(false)
		.title_bar(false)
		.size(window_size, imgui::Condition::Always)
		.position(window_pos, imgui::Condition::Always)
		.begin(ui)
		.unwrap();

	let draw = ui.get_foreground_draw_list();

	draw.add_rect(p0, p1, rgb.0.map(|rgb| rgb as f32 / 255.0)).filled(true).build();

	let pixel_w = state.map.viewport.scale_factor_w.floor();
	let pixel_h = state.map.viewport.scale_factor_h.floor();

	if pixel_w > 1.0 {
		abs_mouse_pos[0] -= abs_mouse_pos[0] % pixel_w;
	}
	if pixel_h > 1.0 {
		abs_mouse_pos[1] -= abs_mouse_pos[1] % pixel_h;
	}

	draw.add_rect(
		[abs_mouse_pos[0] - pixel_w, abs_mouse_pos[1] - pixel_h],
		[abs_mouse_pos[0] + pixel_h, abs_mouse_pos[1] + pixel_h],
		if (rgb.0[0] as f32 * 0.299 + rgb.0[1] as f32 * 0.587 + rgb.0[2] as f32 * 0.114) > 186.0 {
			[0.0, 0.0, 0.0, 1.0]
		} else {
			[1.0, 1.0, 1.0, 1.0]
		},
	)
	.build();

	ui.set_cursor_pos((Point::from(ui.cursor_pos()) + Point::from([0.0, 10.0])).into());

	ui.spacing();

	let font = ui.push_font(state.fonts.debug_small);

	ui.text_wrapped(text);

	font.end();
	window.end();
}
