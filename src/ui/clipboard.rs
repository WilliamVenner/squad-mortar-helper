use clipboard::{ClipboardContext, ClipboardProvider};
use imgui::ClipboardBackend;

pub struct ClipboardSupport(pub ClipboardContext);

#[inline]
pub fn init() -> Result<ClipboardSupport, Box<dyn std::error::Error>> {
	ClipboardContext::new().map(ClipboardSupport)
}

impl ClipboardBackend for ClipboardSupport {
	#[inline]
	fn get(&mut self) -> Option<String> {
		self.0.get_contents().ok()
	}

	#[inline]
	fn set(&mut self, text: &str) {
		let _ = self.0.set_contents(text.to_owned());
	}
}