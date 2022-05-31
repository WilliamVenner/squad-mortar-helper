use super::*;

static INTER: &[u8] = include_bytes!("Inter-Regular.ttf");
static INTER_BOLD: &[u8] = include_bytes!("Inter-Bold.ttf");

pub struct Fonts {
	pub marker_label: FontId,
	pub ocr_label: FontId,
	pub debug_small: FontId
}

#[must_use]
pub fn add(imgui: &mut imgui::Context, hidpi_factor: f64) -> Fonts {
	imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

	let font_size = (14.0 * hidpi_factor) as f32;
	imgui.fonts().add_font(&[
		FontSource::TtfData {
			data: INTER,
			size_pixels: font_size,
			config: Some(FontConfig {
				rasterizer_multiply: 1.5,
				oversample_h: 4,
				oversample_v: 4,
				..FontConfig::default()
			}),
		},

		FontSource::TtfData {
			data: INTER_BOLD,
			size_pixels: font_size,
			config: Some(FontConfig {
				rasterizer_multiply: 1.5,
				oversample_h: 4,
				oversample_v: 4,
				..FontConfig::default()
			}),
		},
	]);

	let font_size = (20.0 * hidpi_factor) as f32;
	let marker_label = imgui.fonts().add_font(&[
		FontSource::TtfData {
			data: INTER_BOLD,
			size_pixels: font_size,
			config: Some(FontConfig {
				rasterizer_multiply: 1.5,
				oversample_h: 4,
				oversample_v: 4,
				..FontConfig::default()
			}),
		}
	]);

	let font_size = (16.0 * hidpi_factor) as f32;
	let ocr_label = imgui.fonts().add_font(&[
		FontSource::TtfData {
			data: INTER_BOLD,
			size_pixels: font_size,
			config: Some(FontConfig {
				rasterizer_multiply: 1.5,
				oversample_h: 4,
				oversample_v: 4,
				..FontConfig::default()
			}),
		}
	]);

	let font_size = (12.0 * hidpi_factor) as f32;
	let debug_small = imgui.fonts().add_font(&[
		FontSource::TtfData {
			data: INTER,
			size_pixels: font_size,
			config: Some(FontConfig {
				rasterizer_multiply: 1.5,
				oversample_h: 4,
				oversample_v: 4,
				..FontConfig::default()
			}),
		}
	]);

	Fonts {
		marker_label,
		ocr_label,
		debug_small
	}
}