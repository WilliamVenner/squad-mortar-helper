use std::sync::{atomic::AtomicBool, Arc};
use core::{cell::UnsafeCell, mem::MaybeUninit};

use atomic_refcell::{AtomicRefCell, AtomicRef, AtomicRefMut};

pub struct DeferCell<T> {
	initialized: AtomicBool,
	value: UnsafeCell<MaybeUninit<T>>
}
impl<T> DeferCell<T> {
	pub const fn defer() -> Self {
		Self {
			initialized: AtomicBool::new(false),
			value: UnsafeCell::new(MaybeUninit::uninit())
		}
	}

	pub unsafe fn set(&self, value: T) {
		assert!(!self.initialized.load(std::sync::atomic::Ordering::Acquire), "already initialized");

		(&mut *self.value.get()).as_mut_ptr().write(value);

		assert!(!self.initialized.swap(true, std::sync::atomic::Ordering::AcqRel), "initialization race");
	}

	pub fn get(&self) -> Option<&T> {
		if self.initialized.load(std::sync::atomic::Ordering::Relaxed) {
			Some(unsafe { (&*self.value.get()).assume_init_ref() })
		} else {
			None
		}
	}
}

unsafe impl<T> Sync for DeferCell<T> {}
unsafe impl<T> Send for DeferCell<T> {}

pub enum ImCellStateRef<'a, T> {
	None,
	Loading,
	Initialized(AtomicRef<'a, T>)
}

pub enum ImCellStateRefMut<'a, T> {
	None,
	Loading,
	Initialized(AtomicRefMut<'a, T>)
}

pub enum ImCellState<L, T> {
	Shutdown,
	None,
	Loading(Option<L>),
	Initialized(T)
}

/// A specialized cell that is designed for asynchronous operations in immediate mode GUIs.
pub struct ImCell<L, T>
where
	T: Send + Sync + 'static,
	L: Send + Sync + 'static
{
	cell: Arc<AtomicRefCell<ImCellState<L, T>>>,
	worker: Option<std::thread::JoinHandle<()>>
}
impl<L, T> ImCell<L, T>
where
	T: Send + Sync + 'static,
	L: Send + Sync + 'static
{
	pub fn new(work: fn(L) -> T, post_work: Option<fn()>) -> Self {
		let cell = Arc::new(AtomicRefCell::new(ImCellState::None));
		Self {
			cell: cell.clone(),

			worker: Some(std::thread::spawn(move || {
				'park: loop {
					std::thread::park();

					let args = loop {
						if let Ok(mut value) = cell.try_borrow_mut() {
							match &mut *value {
								ImCellState::Shutdown => return,
								ImCellState::Loading(args @ Some(_)) => break args.take(),
								_ => continue 'park
							}
						}
						core::hint::spin_loop();
					};

					let result = work(args.unwrap());

					loop {
						if let Ok(mut value) = cell.try_borrow_mut() {
							match &mut *value {
								ImCellState::Shutdown => return,
								ImCellState::None => break,
								#[cfg(debug_assertions)] ImCellState::Initialized(_) => unreachable!(),
								value => {
									*value = ImCellState::Initialized(result);
									break;
								}
							}
						}
						core::hint::spin_loop();
					}

					if let Some(post_work) = post_work {
						post_work();
					}
				}
			}))
		}
	}

	pub fn get(&self) -> ImCellStateRef<'_, T> {
		if let Ok(value) = self.cell.try_borrow() {
			match &*value {
				ImCellState::Shutdown | ImCellState::None => ImCellStateRef::None,
				ImCellState::Loading(_) => ImCellStateRef::Loading,
				ImCellState::Initialized(_) => ImCellStateRef::Initialized(AtomicRef::map(value, |value| match value {
					ImCellState::Initialized(value) => value,
					_ => unsafe { core::hint::unreachable_unchecked() }
				})),
			}
		} else {
			ImCellStateRef::Loading
		}
	}

	pub fn get_mut(&self) -> ImCellStateRefMut<'_, T> {
		if let Ok(value) = self.cell.try_borrow_mut() {
			match &*value {
				ImCellState::Shutdown | ImCellState::None => ImCellStateRefMut::None,
				ImCellState::Loading(_) => ImCellStateRefMut::Loading,
				ImCellState::Initialized(_) => ImCellStateRefMut::Initialized(AtomicRefMut::map(value, |value| match value {
					ImCellState::Initialized(value) => value,
					_ => unsafe { core::hint::unreachable_unchecked() }
				})),
			}
		} else {
			ImCellStateRefMut::Loading
		}
	}

	pub fn load(&self, args: L) {
		loop {
			if let Ok(mut value) = self.cell.try_borrow_mut() {
				*value = ImCellState::Loading(Some(args));
				break;
			}
			core::hint::spin_loop();
		}

		if let Some(ref worker) = self.worker {
			worker.thread().unpark();
		}
	}

	pub fn reset(&self) {
		loop {
			if let Ok(mut value) = self.cell.try_borrow_mut() {
				*value = ImCellState::None;
				break;
			}
			core::hint::spin_loop();
		}
	}
}
impl<L, T> Drop for ImCell<L, T>
where
	T: Send + Sync + 'static,
	L: Send + Sync + 'static
{
	fn drop(&mut self) {
		if let Some(worker) = self.worker.take() {
			loop {
				if let Ok(mut value) = self.cell.try_borrow_mut() {
					*value = ImCellState::Shutdown;
					break;
				}
				core::hint::spin_loop();
			}

			worker.thread().unpark();

			worker.join().ok();
		}
	}
}

/// Spin-lock cell
pub struct SpinCell<T>(AtomicRefCell<T>);
impl<T> SpinCell<T> {
	pub const fn new(inner: T) -> Self {
		Self(AtomicRefCell::new(inner))
	}

	pub fn write(&self) -> AtomicRefMut<'_, T> {
		loop {
			if let Ok(lock) = self.0.try_borrow_mut() {
				return lock;
			}
			core::hint::spin_loop();
		}
	}

	pub fn read(&self) -> AtomicRef<'_, T> {
		loop {
			if let Ok(lock) = self.0.try_borrow() {
				return lock;
			}
			core::hint::spin_loop();
		}
	}
}
impl<T: serde::Serialize> serde::Serialize for SpinCell<T> {
	#[inline]
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer
	{
		self.read().serialize(serializer)
	}
}
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for SpinCell<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>
	{
		Ok(SpinCell::new(T::deserialize(deserializer)?))
	}
}