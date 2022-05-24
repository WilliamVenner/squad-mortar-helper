//! "SMHHM" format
//!
//! * u32 `SMH_MAGIC_NUMBER`
//! * u16 `HEIGHTMAP_FILE_VER`
//! * u32 `SMH_MAGIC_NUMBER`
//! * u32 width
//! * u32 height
//! * liblzma compressed heightmap data

use std::{sync::Arc, io::{Read, Write}};
use smh_heightmap_ripper::Heightmap;
use smh_util::byteorder::{LE, BE, ReadBytesExt, WriteBytesExt};

// Disk-saved heightmap files will be forgotten when these values are changed
pub const SMH_MAGIC_NUMBER: u32 = 0xBADFEEF;
const HEIGHTMAP_FILE_VER: u16 = 0;

pub fn serialize(w: &mut impl Write, heightmap: &Heightmap) -> Result<(), std::io::Error> {
	w.write_u32::<BE>(SMH_MAGIC_NUMBER)?;
	w.write_u16::<LE>(HEIGHTMAP_FILE_VER)?;
	w.write_u32::<BE>(SMH_MAGIC_NUMBER)?;

	w.write_u32::<LE>(heightmap.width)?;
	w.write_u32::<LE>(heightmap.height)?;

	for bound in heightmap.bounds.iter().flatten().copied() {
		w.write_i32::<LE>(bound)?;
	}

	for xyz in heightmap.scale {
		w.write_f32::<LE>(xyz)?;
	}

	for tex_corner in [heightmap.map_tex_corner_0, heightmap.map_tex_corner_1] {
		if let Some(tex_corner) = tex_corner {
			w.write_u8(1)?;
			for xyz in tex_corner {
				w.write_f32::<LE>(xyz)?;
			}
		} else {
			w.write_u8(0)?;
		}
	}

	let mut w = xz2::write::XzEncoder::new(w, 9);
	w.write_all(unsafe { core::slice::from_raw_parts(heightmap.data.as_ptr() as *const u8, heightmap.data.len() * 2) })?;
	w.flush()?;

	Ok(())
}

pub fn deserialize(r: &mut impl Read) -> Result<Option<Heightmap>, std::io::Error> {
	{
		if r.read_u32::<BE>()? != SMH_MAGIC_NUMBER {
			return Ok(None);
		}

		if r.read_u16::<LE>()? != HEIGHTMAP_FILE_VER {
			return Ok(None);
		}

		if r.read_u32::<BE>()? != SMH_MAGIC_NUMBER {
			return Ok(None);
		}
	}

	let width = r.read_u32::<LE>()?;
	let height = r.read_u32::<LE>()?;

	let bounds = [
		[r.read_i32::<LE>()?, r.read_i32::<LE>()?],
		[r.read_i32::<LE>()?, r.read_i32::<LE>()?]
	];

	let scale = [
		r.read_f32::<LE>()?,
		r.read_f32::<LE>()?,
		r.read_f32::<LE>()?
	];

	let map_tex_corner_0 = if r.read_u8()? != 0 {
		Some([r.read_f32::<LE>()?, r.read_f32::<LE>()?, r.read_f32::<LE>()?])
	} else {
		None
	};

	let map_tex_corner_1 = if r.read_u8()? != 0 {
		Some([r.read_f32::<LE>()?, r.read_f32::<LE>()?, r.read_f32::<LE>()?])
	} else {
		None
	};

	let mut data = vec![0u8; width as usize * height as usize * 2];
	xz2::read::XzDecoder::new(r).read_exact(&mut data)?;

	let data = unsafe {
		let transmuted_data = Vec::from_raw_parts(data.as_mut_ptr() as *mut u16, data.len() / 2, data.capacity() / 2);
		std::mem::forget(data);
		transmuted_data
	};

	Ok(Some(Heightmap {
		width,
		height,
		bounds,
		scale,
		map_tex_corner_0,
		map_tex_corner_1,
		data: Arc::from(data)
	}))
}