use super::*;
use atomic_refcell::AtomicRef;
use smh_heightmap_ripper::Heightmap;

const AES_KEY: &str = "0xBC0C07592D6B17BAB88B83A68583A053A6D9A0450CB54ABF5C231DBA59A7466B";

enum LoadedHeightmap {
	Generated(Heightmap),
	Loaded { created: Option<String>, heightmap: Heightmap },
}

fn fmt_system_time(time: SystemTime) -> String {
	chrono::DateTime::<chrono::Local>::from(time).format("%d %b %Y %R").to_string()
}

fn find_workshop_paks(squad_dir: &str) -> Box<[Box<str>]> {
	use std::ffi::OsStr;

	let workshop_dir = Path::new(squad_dir).parent().and_then(|p| p.parent()).map(|p| {
		p.join(format!("workshop/content/{}", smh_heightmap_ripper::SQUAD_APP_ID))
	}).filter(|p| p.is_dir());

	let workshop_dir = match workshop_dir {
		Some(workshop_dir) => workshop_dir.into_boxed_path(),
		None => return Default::default(),
	};

	let mut paks = BTreeSet::new();

	for entry in walkdir::WalkDir::new(workshop_dir) {
		let entry = match entry {
			Ok(entry) => entry,
			Err(err) => {
				log::warn!("Error reading workshop directory: {err}");
				continue
			}
		};

		if entry.file_type().is_file() && entry.path().extension() == Some(OsStr::new("pak")) {
			let parent = match entry.path().parent() {
				Some(parent) => parent,
				None => continue
			};

			if parent.file_name() != Some(OsStr::new("WindowsNoEditor")) {
				continue;
			}

			paks.insert(Box::from(parent.to_string_lossy().as_ref()));
		}
	}

	Vec::from_iter(paks).into_boxed_slice()
}

fn find_squad_dir() -> Option<Box<str>> {
	// I don't want the program to crash because of something as mundane as this
	// so I'm going to wrap it in a `catch_unwind` purely for paranoia reasons
	match std::panic::catch_unwind(smh_heightmap_ripper::find_squad_dir) {
		Ok(squad_dir) => squad_dir.map(|squad_dir| Box::from(squad_dir.to_string_lossy().as_ref())),
		Err(err) => {
			if let Some(err) = err.downcast_ref::<Box<dyn std::error::Error>>() {
				log::error!("Error finding Squad paks dir: {err}");
			} else if let Some(err) = err.downcast_ref::<Box<dyn std::error::Error + Send + Sync + 'static>>() {
				log::error!("Error finding Squad paks dir: {err}");
			} else if let Some(err) = err.downcast_ref::<Box<dyn std::fmt::Display>>() {
				log::error!("Error finding Squad paks dir: {err}");
			} else if let Some(err) = err.downcast_ref::<Box<dyn std::fmt::Debug>>() {
				log::error!("Error finding Squad paks dir: {err:?}");
			} else {
				log::error!("Error finding Squad paks dir: {err:?}");
			}
			None
		}
	}
}

type WorkshopPaks = Box<[Box<str>]>;
struct LoadLayersResult {
	squad_dir: Box<str>,
	aes_key: Box<str>,
	result: Result<(smh_heightmap_ripper::LayersList, WorkshopPaks), smh_heightmap_ripper::Error>,
}
struct LoadLayersOp {
	squad_dir: Box<str>,
	aes_key: Box<str>,
}
impl LoadLayersOp {
	fn load_layers(self) -> LoadLayersResult {
		let main_paks = self.squad_dir.to_string() + "/SquadGame/Content/Paks";
		let workshop_paks = find_workshop_paks(&self.squad_dir);

		let mut result = smh_heightmap_ripper::list_maps(workshop_paks.iter().map(|pak| &**pak).chain([&*main_paks]), Some(&*self.aes_key));
		if let Err(ref err) = result {
			log::warn!("Error loading layers: {}", err);
		}
		if let Ok(layers) = result.as_deref_mut() {
			layers.sort();
		}
		LoadLayersResult {
			squad_dir: self.squad_dir,
			aes_key: self.aes_key,
			result: result.map(|result| (result, workshop_paks))
		}
	}
}

type LoadHeightmapResult = Result<Option<LoadedHeightmap>, smh_heightmap_ripper::Error>;
struct LoadHeightmapOp {
	aes_key: Box<str>,
	paks_dir: Box<str>,
	workshop_paks: WorkshopPaks,
	layer_path: Box<str>,
	skip_cache: bool,
}
impl LoadHeightmapOp {
	fn load_heightmap(self) -> LoadHeightmapResult {
		let mut cache_path = Path::new("heightmaps").join(&*self.layer_path);
		cache_path.set_extension("smhhm");

		if !self.skip_cache && cache_path.is_file() {
			match File::open(&cache_path)
				.map_err(Into::into)
				.and_then(|mut r| squadex::heightmaps::deserialize(&mut r))
			{
				Ok(None) => {}

				Ok(Some(cached)) => {
					return Ok(Some(LoadedHeightmap::Loaded {
						created: cache_path.metadata().and_then(|metadata| metadata.modified()).ok().map(fmt_system_time),
						heightmap: cached,
					}))
				}

				Err(err) => log::warn!("Error opening cached heightmap: {}", err),
			}
		}

		let result = smh_heightmap_ripper::get_heightmap(self.workshop_paks.iter().map(|pak| &**pak).chain([&*self.paks_dir]), Some(&*self.aes_key), &*self.layer_path);
		if let Err(ref err) = result {
			log::warn!("Error generating heightmap for {}: {}", self.layer_path, err);
		}

		if let Ok(Some(ref heightmap)) = result {
			if let Err(err) = std::fs::create_dir_all(cache_path.parent().unwrap()).and_then(|_| {
				File::create(&cache_path)
					.map_err(Into::into)
					.and_then(|mut w| squadex::heightmaps::serialize(&mut w, heightmap))
			}) {
				std::fs::remove_file(&cache_path).ok();
				log::warn!("Error writing heightmap to disk: {}", err);
			}
		}

		result.map(|heightmap| heightmap.map(LoadedHeightmap::Generated))
	}
}

fn color_map_heightmap(heightmap: &Heightmap) -> image::RgbaImage {
	let (width, height, heightmap) = (heightmap.width, heightmap.height, &*heightmap.data);

	let (max, min) = heightmap
		.par_iter()
		.copied()
		.fold(|| (u16::MIN, u16::MAX), |(max, min), val| (val.max(max), val.min(min)))
		.reduce(|| (u16::MIN, u16::MAX), |(max, min), (max_, min_)| (max.max(max_), min.min(min_)));

	let mut colored = image::RgbaImage::new(width, height);
	let par_colored = UnsafeSendPtr::new_mut(&mut colored);
	heightmap.par_iter().copied().enumerate().for_each(|(i, height)| {
		let i = i as u32;
		let x = i % width;
		let y = i / width;

		let color = if height == 0 && min != 0 {
			image::Rgba([0, 0, 0, 0])
		} else {
			// Normalize height
			let height = (height as f64 - min as f64) / (max - min) as f64;

			let r = (height - 0.5).max(0.0) / 0.5;
			let b = ((1.0 - height) - 0.5).max(0.0) / 0.5;
			let g = 1.0 - if height > 0.5 { r } else { b };

			let r = (r * 255.0) as u8;
			let g = (g * 255.0) as u8;
			let b = (b * 255.0) as u8;

			image::Rgba([r, g, b, 255])
		};

		let colored = unsafe { par_colored.clone().as_mut() };
		colored.put_pixel_fast(x, y, color);
	});

	colored
}

fn create_heightmap_texture(
	facade: &Rc<glium::backend::Context>,
	textures: &mut Textures<Texture>,
	heightmap_texture: &mut Option<TextureId>,
	heightmap: &Heightmap,
) -> Result<TextureId, glium::texture::TextureCreationError> {
	let texture = Texture {
		texture: Rc::new(Texture2d::with_format(
			facade,
			RawImage2d {
				width: heightmap.width,
				height: heightmap.height,
				data: Cow::Owned(color_map_heightmap(heightmap).into_raw()),
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

	Ok(if let Some(texture_id) = *heightmap_texture {
		textures.replace(texture_id, texture);
		texture_id
	} else {
		let texture = textures.insert(texture);
		*heightmap_texture = Some(texture);
		texture
	})
}

pub struct HeightmapsUIState {
	opened_heightmaps_folder: bool,

	heightmap_info_fake_input: String,

	default_squad_dir: Option<Box<str>>,
	squad_dir: String,
	aes_key: String,
	filter: String,

	layers: ImCell<LoadLayersOp, LoadLayersResult>,
	heightmap: ImCell<LoadHeightmapOp, LoadHeightmapResult>,
	heightmap_texture: Option<imgui::TextureId>,

	pub selected_heightmap: Option<(imgui::TextureId, [f32; 2], [f32; 2])>,
	pub draw_heightmap: bool,
	pub use_heightmap_offset: bool,

	window_open: bool,
	selected_layer: i32,
}
impl Default for HeightmapsUIState {
	fn default() -> Self {
		// we can get away with resolving the paks dir here
		let default_squad_dir = find_squad_dir();
		Self {
			opened_heightmaps_folder: false,

			heightmap_info_fake_input: String::new(),

			squad_dir: SETTINGS
				.squad_dir()
				.to_owned()
				.map(Into::into)
				.or_else(|| default_squad_dir.as_deref().map(Into::into))
				.unwrap_or_default(),
			default_squad_dir,

			aes_key: SETTINGS.squad_pak_aes().to_owned().map(Into::into).unwrap_or_else(|| AES_KEY.to_string()),

			filter: String::new(),

			layers: ImCell::new(LoadLayersOp::load_layers, Some(ui::redraw)),
			heightmap: ImCell::new(LoadHeightmapOp::load_heightmap, Some(ui::redraw)),
			heightmap_texture: None,
			selected_heightmap: None,
			use_heightmap_offset: true,

			draw_heightmap: Default::default(),
			window_open: Default::default(),
			selected_layer: -1,
		}
	}
}

pub(super) fn menu_bar(state: &mut UIState, ui: &Ui) {
	let menu = match ui.begin_menu("Heightmaps") {
		Some(it) => it,
		_ => return,
	};

	if imgui::MenuItem::new("Select...").build(ui) {
		state.heightmaps.window_open = true;
	}

	let is_set = squadex::heightmaps::is_set();

	if is_set && imgui::MenuItem::new("Clear Selection").build(ui) {
		squadex::heightmaps::set_current(None);

		state.heightmaps.draw_heightmap = false;
		state.heightmaps.selected_heightmap = None;

		if let Some(ref server) = state.web.server {
			server.send(smh_web::Event::Heightmap { heightmap: None });
		}
	}

	if imgui::MenuItem::new("Show Heightmap")
		.enabled(is_set)
		.selected(state.heightmaps.draw_heightmap)
		.build(ui)
	{
		state.heightmaps.draw_heightmap = !state.heightmaps.draw_heightmap;
	}

	// TODO replace with modal "Does this heightmap fit?"
	if imgui::MenuItem::new("Use Heightmap Offset")
		.enabled(is_set)
		.selected(state.heightmaps.use_heightmap_offset)
		.build(ui)
	{
		state.heightmaps.use_heightmap_offset = !state.heightmaps.use_heightmap_offset;
	}

	menu.end();
}

pub(super) fn render_window(state: &mut UIState, ui: &Ui) {
	if !state.heightmaps.window_open {
		// Free memory when the window is closed
		state.heightmaps.heightmap_texture = None;
		state.heightmaps.heightmap.reset();
		return;
	};

	let window = match imgui::Window::new("Heightmaps")
		.collapsible(false)
		.size([275.0, 600.0], imgui::Condition::FirstUseEver)
		.opened(&mut state.heightmaps.window_open)
		.begin(ui)
	{
		Some(window) => window,
		None => {
			state.heightmaps.window_open = false;
			return;
		}
	};

	ui.text("Squad Directory");
	ui.set_next_item_width(-1.0);
	if ui
		.input_text("##SquadDir", &mut state.heightmaps.squad_dir)
		.hint(state.heightmaps.default_squad_dir.as_deref().unwrap_or(""))
		.always_overwrite(false)
		.enter_returns_true(true)
		.build()
	{
		state.heightmaps.layers.reset();
		state.heightmaps.heightmap.reset();
		state.heightmaps.heightmap_texture = None;
		state.heightmaps.selected_layer = -1;

		if state.heightmaps.squad_dir.is_empty() {
			SETTINGS.set_squad_dir(None);

			let squad_dir = find_squad_dir();
			state.heightmaps.default_squad_dir = squad_dir.as_deref().map(Into::into);
			state.heightmaps.squad_dir = squad_dir.map(Into::into).unwrap_or_default();
		} else {
			SETTINGS.set_squad_dir(Some(Box::from(state.heightmaps.squad_dir.as_str())));
		}
	}

	ui.spacing();

	ui.text("AES Key");
	ui.set_next_item_width(-1.0);
	if ui
		.input_text("##AESKey", &mut state.heightmaps.aes_key)
		.hint(AES_KEY)
		.always_overwrite(false)
		.enter_returns_true(true)
		.build()
	{
		state.heightmaps.layers.reset();
		state.heightmaps.heightmap.reset();
		state.heightmaps.heightmap_texture = None;
		state.heightmaps.selected_layer = -1;

		if state.heightmaps.aes_key.is_empty() {
			SETTINGS.set_squad_pak_aes(None);
			state.heightmaps.aes_key = AES_KEY.to_string();
		} else {
			SETTINGS.set_squad_pak_aes(if state.heightmaps.aes_key.trim() == AES_KEY {
				None
			} else {
				Some(Box::from(state.heightmaps.aes_key.as_str()))
			});
		}
	}

	ui.spacing();

	if state.heightmaps.squad_dir.is_empty() {
		ui.text_centered("Squad directory not set. Please input the path to your Squad installation.");
		return;
	}

	// When set to true the heightmap will be regenerated, skipping the cache.
	let mut regenerate = false;

	{
		match state.heightmaps.heightmap.get_mut() {
			ImCellStateRefMut::None => {
				ui.spacing();
				ui.spacing();
				ui.text_centered("Select a layer below to preview its heightmap");
				ui.spacing();
				ui.spacing();
			}
			ImCellStateRefMut::Loading => {
				ui.spacing();
				ui.spacing();
				ui.text_centered("Loading heightmap... this might take a while!");
				ui.spacing();
				ui.spacing();
			}
			ImCellStateRefMut::Initialized(mut heightmap) => match &mut *heightmap {
				Ok(None) => {
					ui.spacing();
					ui.spacing();
					ui.text_centered("This map doesn't have any heightmap data associated with it");
					ui.spacing();
					ui.spacing();
				}
				Ok(opt) => {
					let (heightmap, created) = match &*opt {
						Some(LoadedHeightmap::Generated(heightmap)) => (heightmap, Some(Cow::Owned(fmt_system_time(SystemTime::now())))),
						Some(LoadedHeightmap::Loaded { heightmap, created }) => (heightmap, created.as_deref().map(Cow::Borrowed)),
						None => unsafe { core::hint::unreachable_unchecked() },
					};

					let window = match imgui::ChildWindow::new("HeightmapPreview")
						.draw_background(false)
						.menu_bar(false)
						.movable(false)
						.size([0.0, ui.content_region_avail()[1] / 2.0])
						.begin(ui)
					{
						Some(window) => window,
						None => return,
					};

					let texture = match state.heightmaps.heightmap_texture {
						Some(texture) => Some(texture),
						None => match create_heightmap_texture(state.display.get_context(), state.renderer.textures(), &mut state.heightmaps.heightmap_texture, &*heightmap) {
							Ok(texture) => Some(texture),
							Err(err) => {
								log::error!("Error creating heightmap texture: {}", err);
								None
							}
						},
					};
					if let Some(texture) = texture {
						let [w, h] = ui.window_size();
						let (mut quad, _) = MapViewport::calc(
							w,
							h,
							heightmap.width as f32,
							heightmap.height as f32,
							0,
							Default::default(),
							Default::default(),
						);

						// Offset the quad to the top left of the window
						let [x, y] = ui.window_pos();

						quad.left += x;
						quad.right += x;
						quad.top += y;
						quad.bottom += y;

						ui.get_foreground_draw_list()
							.add_image_quad(texture, quad.top_left(), quad.top_right(), quad.bottom_right(), quad.bottom_left())
							.build();
					}

					window.end();

					if texture.is_some() {
						ui.spacing();
						if ui.button_with_size("SELECT", [-1.0, 0.0]) {
							state.heightmaps.window_open = false;
							state.heightmaps.selected_heightmap = Some((
								state.heightmaps.heightmap_texture.expect("Expected heightmap texture"),
								[heightmap.bounds[0][0] as f32, heightmap.bounds[0][1] as f32],
								[heightmap.width as f32, heightmap.height as f32],
							));

							if let Some(ref server) = state.web.server {
								server.send(smh_web::Event::Heightmap { heightmap: Some(heightmap.clone()) });
							}

							squadex::heightmaps::set_current(Some(match opt.take().sus_unwrap() {
								LoadedHeightmap::Generated(heightmap) => heightmap,
								LoadedHeightmap::Loaded { heightmap, .. } => heightmap,
							}));

							return;
						}
					}

					ui.spacing();
					if ui.button_with_size("Regenerate", [-1.0, 0.0]) {
						regenerate = true;
					}

					ui.spacing();
					ui.spacing();

					if ui.collapsing_header("Heightmap Info", imgui::TreeNodeFlags::NO_TREE_PUSH_ON_OPEN) {
						use std::fmt::Write;

						let font = ui.push_font(state.fonts.debug_small);

						state.heightmaps.heightmap_info_fake_input.clear();

						write!(
							&mut state.heightmaps.heightmap_info_fake_input,
							"Generated: {}\nSize: {}x{} ({:.2} MB)\nScale: {:?}\nMinimap Bounds: {:?}",
							created.unwrap_or(Cow::Borrowed("Unknown")),
							heightmap.width,
							heightmap.height,
							(heightmap.width as usize * heightmap.height as usize * 2) as f32 / 1000000.0,
							heightmap.scale,
							heightmap.bounds
						)
						.ok();

						let info_size = ui.calc_text_size(&state.heightmaps.heightmap_info_fake_input);

						ui.input_text_multiline(
							"##HeightmapInfo",
							&mut state.heightmaps.heightmap_info_fake_input,
							[-1.0, info_size[1] + (ui.frame_padding()[1] * 2.0)],
						)
						.read_only(true)
						.build();

						if ui.button_with_size("Export as PNG", [-1.0, 0.0]) {
							use image::ImageEncoder;

							let compression = if cfg!(debug_assertions) {
								image::png::CompressionType::Fast
							} else {
								image::png::CompressionType::Best
							};

							match std::fs::create_dir_all("heightmaps").map_err(Into::into).and_then(|_| {
								File::create("heightmaps/exported.png").map_err(Into::into).and_then(|f| {
									let data: &[u8] = unsafe { core::slice::from_raw_parts(heightmap.data.as_ptr() as *const u8, heightmap.data.len() * 2) };
									image::codecs::png::PngEncoder::new_with_quality(f, compression, image::png::FilterType::Paeth).write_image(
										data,
										heightmap.width,
										heightmap.height,
										image::ColorType::L16,
									)
								})
							}) {
								Ok(_) => {
									log::info!("Exported PNG heightmap!");

									if !state.heightmaps.opened_heightmaps_folder {
										state.heightmaps.opened_heightmaps_folder = true;
										if let Ok(path) = Path::new("heightmaps").canonicalize() {
											open::that(path).ok();
										}
									}
								},

								Err(err) => log::error!("Error exporting PNG heightmap: {err}"),
							}
						}

						font.end();
					}

					ui.spacing();
				}
				Err(err) => {
					let color = ui.push_style_color(imgui::StyleColor::Text, [1.0, 0.0, 0.0, 1.0]);
					ui.text_wrapped(format!("Error: {}", err));
					color.end();
				}
			},
		}
	}

	{
		let layers = match state.heightmaps.layers.get() {
			ImCellStateRef::Initialized(layers) => {
				if (!state.heightmaps.squad_dir.is_empty() && layers.squad_dir.as_ref() != state.heightmaps.squad_dir.as_str())
					|| (!state.heightmaps.aes_key.is_empty() && layers.aes_key.as_ref() != state.heightmaps.aes_key.as_str())
				{
					drop(layers);

					state.heightmaps.layers.reset();
					state.heightmaps.heightmap.reset();
					state.heightmaps.heightmap_texture = None;
					state.heightmaps.selected_layer = -1;

					None
				} else {
					Some(AtomicRef::map(layers, |layers| &layers.result))
				}
			}
			ImCellStateRef::Loading => None,
			ImCellStateRef::None => {
				state.heightmaps.layers.load(LoadLayersOp {
					squad_dir: Box::from(state.heightmaps.squad_dir.trim()),
					aes_key: Box::from(state.heightmaps.aes_key.trim()),
				});
				None
			}
		};

		ui.spacing();

		ui.align_text_to_frame_padding();
		ui.text("Filter");
		ui.same_line();
		ui.set_next_item_width(-1.0);
		ui.input_text("##Filter", &mut state.heightmaps.filter).build();
		ui.spacing();

		let window = match imgui::ChildWindow::new("LayersList")
			.draw_background(false)
			.menu_bar(false)
			.movable(false)
			.size([0.0, 0.0])
			.begin(ui)
		{
			Some(window) => window,
			None => return,
		};

		match layers.as_deref() {
			Some(Ok((layers, workshop_paks))) => {
				if layers.is_empty() {
					ui.text_centered("No layers found! Maybe your Squad directory or AES key are incorrect?");
				} else {
					let layer_labels = layers
						.iter()
						.enumerate()
						.map(|(i, label)| {
							let label = label.strip_prefix("SquadGame/Content/Maps/").unwrap_or_else(|| label.as_ref());
							let label = label.strip_prefix("SquadGame/Plugins/").unwrap_or(label);
							(i, label)
						});

					let filter = state.heightmaps.filter.trim();
					let (layer_refs, layer_labels): (Vec<usize>, Vec<&str>) = if !filter.is_empty() {
						layer_labels.filter(|(_, label)| label.contains_ignore_ascii_case(filter)).unzip()
					} else {
						layer_labels.unzip()
					};

					if layer_labels.is_empty() {
						ui.text_wrapped("No results found!");
					} else {
						ui.set_next_item_width(-1.0);
						if regenerate || ui.list_box("", &mut state.heightmaps.selected_layer, &*layer_labels, layer_labels.len() as i32) {
							state.heightmaps.heightmap.reset();
							state.heightmaps.heightmap_texture = None;

							if (0..layer_labels.len() as i32).contains(&state.heightmaps.selected_layer) {
								state.heightmaps.heightmap.load(LoadHeightmapOp {
									aes_key: Box::from(state.heightmaps.aes_key.trim()),
									paks_dir: (state.heightmaps.squad_dir.trim().to_owned() + "/SquadGame/Content/Paks").into_boxed_str(),
									workshop_paks: workshop_paks.clone(),
									layer_path: layers[layer_refs[state.heightmaps.selected_layer as usize]].clone(),
									skip_cache: regenerate,
								});
							}
						}
					}
				}
			}

			Some(Err(err)) => {
				let color = ui.push_style_color(imgui::StyleColor::Text, [1.0, 0.0, 0.0, 1.0]);
				ui.text_wrapped(format!("Error: {}", err));
				color.end();
			}

			None => ui.text_wrapped("Discovering..."),
		}

		window.end();
	}

	window.end();
}

pub(super) fn render_overlay(state: &mut UIState, ui: &Ui) {
	if state.heightmaps.draw_heightmap {
		if let Some((texture_id, offset, [width, height])) = state.heightmaps.selected_heightmap {
			if let Some(minimap_viewport) = state.vision.minimap_bounds {
				// To position the heightmap:
				// 1. Add offset to the heightmap position, anchoring the heightmap to the bottom right corner
				// 2. Scale the heightmap to the minimap size

				let offset = if state.heightmaps.use_heightmap_offset {
					let hm_scale_factor_w = (minimap_viewport.right - minimap_viewport.left) as f32 / (width + offset[0]);
					let hm_scale_factor_h = (minimap_viewport.bottom - minimap_viewport.top) as f32 / (height + offset[1]);
					[offset[0] * hm_scale_factor_w * state.map.viewport.scale_factor_w, offset[1] * hm_scale_factor_h * state.map.viewport.scale_factor_h]
				} else {
					[0.0, 0.0]
				};

				let minimap_viewport = Rect {
					left: state.map.viewport.translate_x(minimap_viewport.left as f32) + offset[0],
					top: state.map.viewport.translate_y(minimap_viewport.top as f32) + offset[1],
					right: state.map.viewport.translate_x(minimap_viewport.right as f32),
					bottom: state.map.viewport.translate_y(minimap_viewport.bottom as f32),
				};

				// cheating really
				ui.set_cursor_pos(minimap_viewport.top_left());

				imgui::Image::new(texture_id, [minimap_viewport.width(), minimap_viewport.height()])
					.tint_col([1.0, 1.0, 1.0, 0.25])
					.build(ui);
			}
		}
	}
}