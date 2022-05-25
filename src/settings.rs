use std::sync::atomic::*;
use smh_util::{SpinCell, atomic_refcell::AtomicRef};

macro_rules! atomic_types {
	{$($ty:ty => $inner:ty),*} => {
		pub trait AtomicType {
			type AtomicType;
		}
		$(impl AtomicType for $ty {
			type AtomicType = $inner;
		})*
	};
}

atomic_types! {
	AtomicBool => bool,
	AtomicU8 => u8,
	AtomicI8 => i8,
	AtomicU16 => u16,
	AtomicI16 => i16,
	AtomicU32 => u32,
	AtomicI32 => i32,
	AtomicU64 => u64,
	AtomicI64 => i64
}

macro_rules! settings {
	{atomics => {$($(#[$attr:meta])? $name:ident: $ty:ty = $default:expr),*}, spinners => {$($(#[$spin_attr:meta])? $spin_name:ident: $spin_ty:ty = $spin_default:expr),*}} => {
		#[derive(serde::Serialize, serde::Deserialize)]
		pub struct SpinSettings {
			$($(#[$spin_attr])? $spin_name: SpinCell<$spin_ty>,)*
		}
		impl SpinSettings {
			fn default() -> Self {
				Self {
					$($(#[$spin_attr])? $spin_name: SpinCell::new($spin_default)),*
				}
			}
		}

		#[derive(serde::Serialize, serde::Deserialize)]
		pub struct Settings {
			#[serde(flatten)]
			spinners: SpinSettings,

			$($(#[$attr])? $name: $ty,)*
		}
		impl Settings {
			fn default() -> Self {
				Self {
					spinners: SpinSettings::default(),
					$($(#[$attr])? $name: <$ty>::new($default)),*
				}
			}

			fn save(&self) {
				if let Ok(settings) = serde_json::to_string_pretty(self) {
					std::fs::write("settings.json", settings).ok();
				}
			}

			fn load() -> Self {
				std::fs::File::open("settings.json").ok().and_then(|f| {
					serde_json::from_reader(f).ok()
				})
				.unwrap_or_else(|| {
					Self::default()
				})
			}

			$(
				$(#[$attr])?
				pub fn $name(&self) -> <$ty as AtomicType>::AtomicType {
					self.$name.load(Ordering::Relaxed)
				}

				smh_util::paste::paste! {
					$(#[$attr])?
					pub fn [< set_ $name >] (&self, val: <$ty as AtomicType>::AtomicType) {
						self.$name.store(val, Ordering::Release);
						self.save();
					}
				}
			)*

			$(
				$(#[$spin_attr])?
				pub fn $spin_name(&self) -> AtomicRef<'_, $spin_ty> {
					self.spinners.$spin_name.read()
				}

				smh_util::paste::paste! {
					$(#[$spin_attr])?
					pub fn [< set_ $spin_name >] (&self, val: $spin_ty) {
						*self.spinners.$spin_name.write() = val;
						self.save();
					}
				}
			)*
		}

		magic_statics_mod! {
			pub static ref SETTINGS: Settings = Settings::load();
		}
	}
}

settings! {
	atomics => {
		#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))]
		hardware_acceleration: AtomicBool = true,
		github_star_modal: AtomicU8 = 0,
		detect_markers: AtomicBool = true,
		grayscale_map: AtomicBool = true
	},

	spinners => {
		squad_dir: Option<Box<str>> = None,
		squad_pak_aes: Option<Box<str>> = None
	}
}

pub fn menu_bar(ui: &imgui::Ui) {
	if let Some(settings) = ui.begin_menu("Settings") {
		#[cfg(all(feature = "gpu", any(windows, target_os = "linux"), target_arch = "x86_64"))] {
			let hardware_acceleration = SETTINGS.hardware_acceleration();
			if imgui::MenuItem::new("Hardware Acceleration (GPU)").selected(hardware_acceleration).build(ui) {
				SETTINGS.set_hardware_acceleration(!hardware_acceleration);
			}
		}

		let detect_markers = SETTINGS.detect_markers();
		if imgui::MenuItem::new("Detect Markers").selected(detect_markers).build(ui) {
			SETTINGS.set_detect_markers(!detect_markers);
		}

		let grayscale_map = SETTINGS.grayscale_map();
		if imgui::MenuItem::new("Grayscale Map").selected(grayscale_map).build(ui) {
			SETTINGS.set_grayscale_map(!grayscale_map);
		}

		settings.end();
	}
}
