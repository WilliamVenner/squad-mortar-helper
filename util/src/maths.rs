use core::ops::*;

pub trait IntDiv: Sized + Add<Output = Self> + Div<Output = Self> + Sub<Output = Self> {
	fn int_div_ceil(self, rhs: Self) -> Self;
}
macro_rules! impl_int_div {
	($($ty:ty),*) => {
		$(impl IntDiv for $ty {
			fn int_div_ceil(self, rhs: Self) -> Self {
				(self + rhs - 1) / rhs
			}
		})*
	};
}
impl_int_div!(usize, i32, u32, i64, u64, i8, u8, i16, u16);