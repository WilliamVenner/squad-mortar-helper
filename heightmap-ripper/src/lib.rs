use std::{
	io::Cursor,
	path::PathBuf,
	process::Command, sync::Arc,
};

use byteorder::{ReadBytesExt, LE};

pub type LayersList = Box<[Box<str>]>;

#[derive(Clone)]
pub struct Heightmap {
	pub width: u32,
	pub height: u32,
	pub bounds: [[i32; 2]; 2],
	pub scale: [f32; 3],
	pub map_tex_corner_0: Option<[f32; 3]>,
	pub map_tex_corner_1: Option<[f32; 3]>,
	pub data: Arc<[u16]>,
}
impl Heightmap {
	#[inline]
	pub fn as_image(&self) -> image::ImageBuffer<image::Luma<u16>, &[u16]> {
		image::ImageBuffer::from_raw(self.width, self.height, &*self.data).unwrap()
	}

	#[inline]
	pub fn get_height(&self, x: u32, y: u32) -> Option<u16> {
		let idx = ((y as i32 + self.bounds[0][1]) * self.width as i32) + (x as i32 + self.bounds[0][0]);
		self.data.get::<usize>(idx.try_into().ok()?).copied()
	}
}
impl core::ops::Index<(u32, u32)> for Heightmap {
	type Output = u16;

	#[inline]
	fn index(&self, (x, y): (u32, u32)) -> &Self::Output {
		&self.data[(y * self.width + x) as usize]
	}
}
impl core::fmt::Debug for Heightmap {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Heightmap")
			.field("width", &self.width)
			.field("height", &self.height)
			.field("bounds", &self.bounds)
			.field("scale", &self.scale)
			.field("map_tex_corner_0", &self.map_tex_corner_0)
			.field("map_tex_corner_1", &self.map_tex_corner_1)
			.field("data", &self.data.len())
			.finish()
	}
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),

	#[error("{0}")]
	Runtime(Box<str>),
}

pub fn find_squad_dir() -> Option<PathBuf> {
	const SQUAD_APP_ID: u32 = 393380;
	steamlocate::SteamDir::locate()?.app(&SQUAD_APP_ID).map(|app| app.path.to_path_buf())
}

pub fn get_heightmap(paks_dir: impl AsRef<str>, aes_key: Option<impl AsRef<str>>, map_path: impl AsRef<str>) -> Result<Option<Heightmap>, Error> {
	log::info!("Generating heightmap...");
	log::info!("Map: {}", map_path.as_ref());
	log::info!("PAKs: {}", paks_dir.as_ref());
	log::info!("AES key: {:?}", aes_key.as_ref().map(|aes_key| aes_key.as_ref()));

	let mut cmd = Command::new("SquadHeightmapRipper");

	cmd.arg("-p").arg(paks_dir.as_ref());
	cmd.arg("-m").arg(map_path.as_ref());

	if let Some(aes_key) = aes_key {
		cmd.arg("-k").arg(aes_key.as_ref());
	}

	let output = cmd.output()?;
	if !output.status.success() {
		return Err(Error::Runtime(
			format!(
				"Status: {:?}\n\n======= STDOUT =======\n{}\n\n======= STDERR =======\n{}",
				output.status,
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr)
			)
			.into_boxed_str(),
		));
	}

	let mut output = Cursor::new(output.stdout);

	let width = output.read_u32::<LE>()?;
	let height = output.read_u32::<LE>()?;

	if width == 0 && height == 0 {
		log::info!("Heightmap has no data (width 0, height 0)");
		return Ok(None);
	} else {
		log::info!("Heightmap size {width}x{height}");
	}

	let bounds = [
		[output.read_i32::<LE>()?, output.read_i32::<LE>()?],
		[output.read_i32::<LE>()?, output.read_i32::<LE>()?]
	];

	let scale = [output.read_f32::<LE>()?, output.read_f32::<LE>()?, output.read_f32::<LE>()?];

	let map_tex_corner_0 = if output.read_u8()? != 0 {
		Some([output.read_f32::<LE>()?, output.read_f32::<LE>()?, output.read_f32::<LE>()?])
	} else {
		None
	};

	let map_tex_corner_1 = if output.read_u8()? != 0 {
		Some([output.read_f32::<LE>()?, output.read_f32::<LE>()?, output.read_f32::<LE>()?])
	} else {
		None
	};

	Ok(Some(Heightmap {
		width,
		height,
		bounds,
		scale,
		map_tex_corner_0,
		map_tex_corner_1,
		data: {
			let pos = output.position() as usize;
			let output = output.into_inner();
			let output = &output[pos..];

			// If the output is completely blank, it means this layer doesn't have a heightmap
			if !output.iter().copied().any(|byte| byte != 0) {
				log::info!("Heightmap has no data (all zero)");
				return Ok(None);
			}

			let mut data = Vec::with_capacity((output.len() / 2) as usize);

			let (prefix, shorts, suffix) = unsafe { output.align_to::<u16>() };

			// Copy prefix into data
			prefix
				.chunks_exact(2)
				.map(|bytes| [bytes[0], bytes[1]])
				.map(u16::from_le_bytes)
				.for_each(|height| data.push(height));

			// Faster memcpy
			data.extend_from_slice(shorts);

			// Copy suffix into data
			suffix
				.chunks_exact(2)
				.map(|bytes| [bytes[0], bytes[1]])
				.map(u16::from_le_bytes)
				.for_each(|height| data.push(height));

			Arc::from(data)
		},
	}))
}

pub fn list_maps(paks_dir: impl AsRef<str>, aes_key: Option<impl AsRef<str>>) -> Result<Box<[Box<str>]>, Error> {
	log::info!("Listing maps...");
	log::info!("PAKs: {}", paks_dir.as_ref());
	log::info!("AES key: {:?}", aes_key.as_ref().map(|aes_key| aes_key.as_ref()));

	let mut cmd = Command::new("SquadHeightmapRipper");

	cmd.arg("-p").arg(paks_dir.as_ref());

	if let Some(aes_key) = aes_key {
		cmd.arg("-k").arg(aes_key.as_ref());
	}

	let output = cmd.output()?;
	if !output.status.success() {
		return Err(Error::Runtime(
			format!(
				"Status: {:?}\n\n======= STDOUT =======\n{}\n\n======= STDERR =======\n{}",
				output.status,
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr)
			)
			.into_boxed_str(),
		));
	}

	let output = output
		.stdout
		.split(|&b| b == b'\n')
		.filter_map(|line| {
			line.last().copied().and_then(|last| {
				if last == b'\r' {
					if line.len() > 1 {
						Some(&line[0..line.len() - 1])
					} else {
						None
					}
				} else {
					Some(line)
				}
			})
		})
		.filter_map(|line| std::str::from_utf8(line).ok())
		.filter(|line| {
			![
				"/Lighting_Layers/",
				"/Sound_Layers/",
				"/VFX_Layers/",
				"/FX_Layers/",
				"/Gameplay_Layers/",
				"/VFX_Sound_Layers/",
			]
			.into_iter()
			.any(|filter| line.contains(filter))
		})
		.map(Box::from)
		.collect::<Box<[Box<str>]>>();

	log::info!("Discovered {} maps", output.len());

	Ok(output)
}

#[test]
fn test_get_heightmap() {
	use image::buffer::ConvertBuffer;

	let heightmap = get_heightmap(
		r#"Q:\Steam\steamapps\common\Squad\SquadGame\Content\Paks"#,
		Some("0xBC0C07592D6B17BAB88B83A68583A053A6D9A0450CB54ABF5C231DBA59A7466B"),
		"SquadGame/Content/Maps/Mutaha/Mutaha.umap",
	)
	.unwrap()
	.unwrap();

	let heightmap: image::RgbImage = heightmap.as_image().convert();
	let path = std::env::temp_dir().join("heightmap.png");
	heightmap.save_with_format(&path, image::ImageFormat::Png).unwrap();
	open::that(path).unwrap();
}

#[test]
fn test_list_maps() {
	println!(
		"{:#?}",
		list_maps(
			r#"Q:\Steam\steamapps\common\Squad\SquadGame\Content\Paks"#,
			Some("0xBC0C07592D6B17BAB88B83A68583A053A6D9A0450CB54ABF5C231DBA59A7466B")
		)
		.unwrap()
	);
}
