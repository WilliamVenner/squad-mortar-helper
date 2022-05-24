use core::mem::MaybeUninit;

#[repr(transparent)]
pub struct SmallVecIter<T, const N: usize>(core::iter::Take<core::array::IntoIter<MaybeUninit<T>, N>>);
impl<T, const N: usize> Iterator for SmallVecIter<T, N> {
	type Item = T;

	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		self.0.next().map(|item| unsafe { item.assume_init() })
	}
}

#[derive(Debug)]
pub struct SmallVec<T, const N: usize> {
	container: [MaybeUninit<T>; N],
	count: usize,
}
impl<T, const N: usize> SmallVec<T, N> {
	#[inline]
	pub fn new() -> Self {
		Self {
			container: unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() },
			count: 0
		}
	}

	#[inline]
	pub fn is_full(&self) -> bool {
		self.len() == N
	}

	#[inline]
	pub fn push(&mut self, val: T) {
		assert!(self.count < N, "SmallVec is full");
		self.container[self.count] = MaybeUninit::new(val);
		self.count += 1;
	}

	#[inline]
	pub fn as_slice(&self) -> &[T] {
		unsafe { core::slice::from_raw_parts(self.container.as_ptr() as *const T, self.count) }
	}

	#[inline]
	pub fn as_slice_mut(&mut self) -> &mut [T] {
		unsafe { core::slice::from_raw_parts_mut(self.container.as_mut_ptr() as *mut T, self.count) }
	}
}
impl<T: Clone, const N: usize> Clone for SmallVec<T, N> where MaybeUninit<T>: Clone {
	#[inline]
    fn clone(&self) -> Self {
        Self {
			container: self.container.clone(),
			count: self.count
		}
    }
}
impl<T, const N: usize> Default for SmallVec<T, N> {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}
impl<T, const N: usize> IntoIterator for SmallVec<T, N> {
	type Item = T;

	type IntoIter = SmallVecIter<T, N>;

	#[inline]
	fn into_iter(self) -> Self::IntoIter {
		SmallVecIter(self.container.into_iter().take(self.count))
	}
}
impl<T, const N: usize> core::ops::Deref for SmallVec<T, N> {
	type Target = [T];

	#[inline]
	fn deref(&self) -> &Self::Target {
		self.as_slice()
	}
}
impl<T, const N: usize> core::ops::DerefMut for SmallVec<T, N> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.as_slice_mut()
	}
}
impl<'a, T, const N: usize> IntoIterator for &'a SmallVec<T, N> {
	type Item = &'a T;

	type IntoIter = core::slice::Iter<'a, T>;

	#[inline]
	fn into_iter(self) -> Self::IntoIter {
		self.as_slice().iter()
	}
}