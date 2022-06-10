use super::*;

static INTER: &[u8] = include_bytes!("Inter-Regular.ttf");
static INTER_BOLD: &[u8] = include_bytes!("Inter-Bold.ttf");

macro_rules! fonts {
	($($name:ident: $font:ident, $size:literal;)*) => {
		pub(super) struct Fonts {
			$(pub(super) $name: FontId,)*
		}
		impl Fonts {
			#[must_use]
			pub(super) fn add(imgui: &mut imgui::Context, hidpi_factor: f64) -> Self {
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

				let mut fonts = imgui.fonts();
				Self {
					$($name: fonts.add_font(&[
						FontSource::TtfData {
							data: $font,
							size_pixels: ($size * hidpi_factor) as f32,
							config: Some(FontConfig {
								rasterizer_multiply: 1.5,
								oversample_h: 4,
								oversample_v: 4,
								..FontConfig::default()
							}),
						}
					]),)*
				}
			}
		}
	};
}
fonts! {
	marker_label: INTER_BOLD, 20.0;
	ocr_label: INTER_BOLD, 16.0;
	debug_small: INTER, 12.0;
	paused: INTER_BOLD, 26.0;
}