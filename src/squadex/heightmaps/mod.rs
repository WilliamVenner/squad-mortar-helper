use crate::*;
use smh_heightmap_ripper::Heightmap;
use atomic_refcell::AtomicRef;

mod serde;
pub use self::serde::{deserialize, serialize};

static ACTIVE_HEIGHTMAP: SpinCell<Option<Heightmap>> = SpinCell::new(None);

#[inline]
pub fn is_set() -> bool {
	ACTIVE_HEIGHTMAP.read().is_some()
}

#[inline]
pub fn get_current() -> Option<AtomicRef<'static, Heightmap>> {
	let hm = ACTIVE_HEIGHTMAP.read();
	if hm.is_none() {
		None
	} else {
		Some(AtomicRef::map(hm, |hm| {
			hm.as_ref().sus_unwrap()
		}))
	}
}

#[inline]
pub fn set_current(heightmap: Option<Heightmap>) {
	*ACTIVE_HEIGHTMAP.write() = heightmap;
}