pub trait ImguiEx {
	fn text_centered(&self, text: impl AsRef<str>);
	fn button_centered(&self, text: impl AsRef<str>) -> bool;
	fn frame_padding(&self) -> [f32; 2];
	fn window_padding(&self) -> [f32; 2];
}
impl ImguiEx for imgui::Ui<'_> {
	#[inline]
	fn text_centered(&self, text: impl AsRef<str>) {
		let win_width = self.window_size()[0];
		let text_width = self.calc_text_size(text.as_ref())[0];

		let text_indentation = ((win_width - text_width) * 0.5).max(0.0);

		self.same_line_with_pos(text_indentation);
		let wrap = self.push_text_wrap_pos_with_pos(win_width - text_indentation);
		self.text_wrapped(text.as_ref());
		wrap.pop(self);
	}

	#[inline]
	fn button_centered(&self, label: impl AsRef<str>) -> bool {
		let size = self.calc_text_size(label.as_ref())[0] + self.frame_padding()[0] * 2.0;
		let avail = self.content_region_avail()[0];

		let off = (avail - size) * 0.5;
		if off > 0.0 {
			let [x, y] = self.cursor_pos();
			self.set_cursor_pos([x + off, y]);
		}

		self.button(label.as_ref())
	}

	#[inline]
	fn frame_padding(&self) -> [f32; 2] {
		let padding = unsafe { (*imgui::sys::igGetStyle()).FramePadding };
		[padding.x, padding.y]
	}

	#[inline]
	fn window_padding(&self) -> [f32; 2] {
		let padding = unsafe { (*imgui::sys::igGetStyle()).WindowPadding };
		[padding.x, padding.y]
	}
}