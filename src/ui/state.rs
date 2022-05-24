use super::*;

pub(super) struct UIState {
	pub star_modal: bool,

	pub ui_fmt_alloc: bumpalo::Bump,

	pub fonts: Fonts,
	pub logs: logs::LogState,

	pub debug: debug::DebugState,
	pub draw: draw::DrawState,
	pub map: map::MapState,
	pub web: web::WebState,
	pub heightmaps: heightmaps::HeightmapsUIState,

	pub new_data: bool,
	pub frame: u64,
	pub display_size: [f32; 2],

	pub display: Display,
	pub renderer: &'static mut Renderer,

	pub(super) vision: FrameCell<parking_lot::MappedMutexGuard<'static, UIData>>,
}
impl UIState {
	pub(super) fn new(display: Display, renderer: &'static mut Renderer, fonts: Fonts, logs: logs::LogState) -> Self {
		Self {
			star_modal: false,

			ui_fmt_alloc: Default::default(),

			debug: Default::default(),
			draw: Default::default(),
			web: Default::default(),
			heightmaps: Default::default(),
			map: Default::default(),
			logs,
			fonts,

			new_data: true,
			frame: 0,
			display_size: [0.0, 0.0],

			vision: Default::default(),

			display,
			renderer
		}
	}

	pub(super) fn render(&mut self, ui: &Ui) {
		let window = imgui::Window::new("Main")
			.no_decoration()
			.bg_alpha(0.)
			.movable(false)
			.resizable(false)
			.position([0., 0.], imgui::Condition::Always)
			.size(self.display_size, imgui::Condition::Always)
			.collapsible(false)
			.draw_background(false)
			.menu_bar(true)
			.bring_to_front_on_focus(false)
			.begin(ui)
			.unwrap();

		if self.new_data {
			if let Some(ref server) = self.web.server {
				server.send(smh_web::Event::UpdateState {
					meters_to_px_ratio: self.vision.meters_to_px_ratio,
					minimap_bounds: self.vision.minimap_bounds
				});
			}
		}

		web::handle_interactions(self);

		if let Some(menu) = ui.begin_menu_bar() {
			heightmaps::menu_bar(self, ui);
			settings::menu_bar(ui);
			web::menu_bar(self, ui);
			debug::menu_bar(ui, self);
			about::menu_bar(ui);

			menu.end();
		}

		map::render(self, ui);
		debug::render(self, ui);

		heightmaps::render_window(self, ui);
		logs::render_window(self, ui);
		web::render_popup(self, ui);
		about::render_star_pls(self, ui);

		window.end();
	}
}

pub(super) struct FrameCell<T>(Option<T>);
impl<T> FrameCell<T> {
	#[inline]
	pub(super) fn set(&mut self, val: T) {
		self.0 = Some(val);
	}

	#[inline]
	pub(super) fn take(&mut self) -> Option<T> {
		self.0.take()
	}
}
impl<T> Default for FrameCell<T> {
	#[inline]
	fn default() -> Self {
		FrameCell(None)
	}
}
impl<T> core::ops::Deref for FrameCell<T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		self.0.as_ref().sus_unwrap()
	}
}
impl<T> core::ops::DerefMut for FrameCell<T> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.as_mut().sus_unwrap()
	}
}