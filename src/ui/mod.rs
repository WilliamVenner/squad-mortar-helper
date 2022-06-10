use crate::prelude::*;

use glium::{
	backend::Facade,
	glutin::{
		self,
		event::{Event, WindowEvent},
		event_loop::{ControlFlow, EventLoop, EventLoopProxy},
		window::WindowBuilder,
	},
	texture::RawImage2d,
	uniforms::SamplerBehavior,
	Display, Surface, Texture2d,
};
use imgui::{Context, FontConfig, FontId, FontSource, TextureId, Textures, Ui};
use imgui_glium_renderer::{Renderer, Texture};
use imgui_winit_support::{HiDpiMode, WinitPlatform};

macro_rules! relative {
	($ui:ident, $coords:expr) => {{
		let o = $ui.cursor_screen_pos();
		[$coords[0] + o[0], $coords[1] + o[1]]
	}};

	($ui:ident, $x:expr, $y:expr) => {{
		let o = $ui.cursor_screen_pos();
		[$x + o[0], $y + o[1]]
	}};
}

#[derive(Copy, Clone)]
pub enum DrawList {
	#[allow(unused)]
	Window,
	#[allow(unused)]
	Foreground,
	#[allow(unused)]
	Background,
}
impl DrawList {
	#[inline]
	unsafe fn as_ptr(self) -> *mut imgui::sys::ImDrawList {
		match self {
			Self::Window => imgui::sys::igGetWindowDrawList(),
			Self::Foreground => imgui::sys::igGetForegroundDrawList(),
			Self::Background => imgui::sys::igGetBackgroundDrawList(),
		}
	}

	#[inline]
	fn get<'a>(self, ui: &'a imgui::Ui<'a>) -> imgui::DrawListMut<'a> {
		match self {
			Self::Window => ui.get_window_draw_list(),
			Self::Foreground => ui.get_foreground_draw_list(),
			Self::Background => ui.get_background_draw_list(),
		}
	}
}

mod fmt;
use fmt::ui_format;

pub mod heightmaps;
pub mod logs;
pub mod debug;

mod about;
mod clipboard;
mod draw;
mod fonts;
mod map;
mod markers;
mod rotate;
mod theme;
mod web;
mod window;
mod state;
mod update;
mod settings;

#[path = "imgui.rs"]
mod imgui_ex;

use fonts::Fonts;
use imgui_ex::ImguiEx;
use map::MapViewport;
use rotate::ImRotate;
use state::UiState;
pub use window::start;

pub static DPI_ESTIMATE: AtomicU32 = AtomicU32::new(0);

magic_statics_mod! {
	static ref UI_DATA: parking_lot::Mutex<(usize, UiData)> = Default::default();
}
pub fn update<F: FnOnce(&mut UiData)>(f: F) {
	let mut ui_data = UI_DATA.lock();

	ui_data.0 = ui_data.0.wrapping_add(1);
	f(&mut ui_data.1);

	redraw();
}

enum UIEvent {
	Redraw,
	Shutdown,
}

static TX: DeferCell<EventLoopProxy<UIEvent>> = DeferCell::defer();
pub fn redraw() {
	if let Some(tx) = TX.get() {
		let _ = tx.send_event(UIEvent::Redraw);
	}
}
pub fn shutdown() {
	if let Some(tx) = TX.get() {
		log::info!("shutting down ui...");
		let _ = tx.send_event(UIEvent::Shutdown);
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Marker {
	pub p0: [f32; 2],
	pub p1: [f32; 2],
	pub meters: Option<f64>,
}
impl Marker {
	#[inline]
	pub fn new(p0: [f32; 2], p1: [f32; 2], meters_to_px_ratio: Option<f64>) -> Self {
		Self {
			p0,
			p1,
			meters: meters_to_px_ratio.map(|meters_to_px_ratio| {
				let length = ((p0[0] as f64 - p1[0] as f64).powi(2) + (p0[1] as f64 - p1[1] as f64).powi(2)).sqrt();
				length * meters_to_px_ratio
			}),
		}
	}
}

#[derive(Default, Debug)]
pub struct UiData {
	pub sleeping: bool,
	pub markers: Box<[Marker]>,
	pub map: Arc<image::RgbaImage>,
	pub minimap_bounds: Option<Rect<u32>>,
	pub meters_to_px_ratio: Option<f64>,
	pub debug: DebugBox,
}