//! On unsupported operating systems, return nothing

use super::SquadEx;

#[inline]
pub fn get() -> Option<SquadEx> {
	None
}