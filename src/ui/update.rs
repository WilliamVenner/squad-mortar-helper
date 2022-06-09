use super::*;

fn check() -> Option<Box<str>> {
	#[derive(serde::Deserialize)]
	struct GitHubTag {
		name: Box<str>,
	}

	let installed_ver = match semver::Version::parse(env!("CARGO_PKG_VERSION")) {
		Ok(installed_ver) => installed_ver,
		Err(err) => {
			if cfg!(debug_assertions) {
				panic!("Failed to parse installed version: {err:#?}");
			} else {
				log::error!("Failed to parse installed version: {err:#?}");
				return None;
			}
		}
	};

	let data = match sysreq::get("https://api.github.com/repos/WilliamVenner/squad-mortar-helper/tags") {
		Ok(data) => data,
		Err(err) => {
			log::error!("Error checking for updates: {err:#?}");
			return None;
		}
	};

	let tags: Box<[GitHubTag]> = match serde_json::de::from_slice(&data) {
		Ok(tags) => tags,
		Err(err) => {
			log::error!("Error parsing GitHub tags: {err:#?}");
			return None;
		}
	};

	let latest_tag = match tags
		.iter()
		.filter_map(|tag| if tag.name.starts_with('v') { Some(&tag.name[1..]) } else { None })
		.filter_map(|ver| semver::Version::parse(ver).ok())
		.max()
	{
		Some(ver) => ver,
		None => {
			log::error!("Error checking for updates: no tags found");
			return None;
		}
	};

	if latest_tag > installed_ver {
		log::info!("New version available: v{latest_tag}");
		Some(
			format!(
				concat!(
					"A new version of Squad Mortar Helper is available!\nYou're using v",
					env!("CARGO_PKG_VERSION"),
					", but v{} has been released!"
				),
				latest_tag
			)
			.into_boxed_str(),
		)
	} else {
		log::info!("Up to date!");
		None
	}
}

static UPDATE_CHECK_DONE: AtomicBool = AtomicBool::new(false);

pub(super) enum UpdateCheckState {
	Pending(std::thread::JoinHandle<Option<Box<str>>>),
	Done { latest_version: Option<Box<str>>, modal_opened: bool },
}
impl Default for UpdateCheckState {
	#[inline]
	fn default() -> Self {
		Self::Pending(std::thread::spawn(move || {
			let res = check();
			UPDATE_CHECK_DONE.store(true, std::sync::atomic::Ordering::Release);
			res
		}))
	}
}

pub(super) fn render_modal(state: &mut UIState, ui: &Ui) {
	let latest_version = match &mut state.update_check {
		UpdateCheckState::Done {
			latest_version,
			modal_opened,
		} => latest_version.as_deref().map(|latest_version| (latest_version, modal_opened)),

		UpdateCheckState::Pending(_) => {
			if UPDATE_CHECK_DONE.load(std::sync::atomic::Ordering::Acquire) {
				let thread = match core::mem::replace(
					&mut state.update_check,
					UpdateCheckState::Done {
						latest_version: None,
						modal_opened: false,
					},
				) {
					UpdateCheckState::Pending(thread) => thread,
					_ => unreachable!(),
				};

				match &mut state.update_check {
					UpdateCheckState::Done {
						latest_version,
						modal_opened,
					} => {
						*latest_version = thread.join().ok().flatten();
						latest_version.as_deref().map(|latest_version| (latest_version, modal_opened))
					}
					_ => unreachable!(),
				}
			} else {
				None
			}
		}
	};

	let (latest_version, modal_opened) = match latest_version {
		Some(latest_version) => latest_version,
		None => return,
	};

	if !*modal_opened {
		*modal_opened = true;
		ui.open_popup("Update Available!");
	}

	if let Some(modal) = imgui::PopupModal::new("Update Available!").resizable(false).begin_popup(ui) {
		ui.text(latest_version);
		ui.spacing();

		if ui.button("Download") {
			open::that("https://github.com/WilliamVenner/squad-mortar-helper/releases/latest").ok();
			ui.close_current_popup();
		}

		ui.same_line();

		if ui.button("Dismiss") {
			ui.close_current_popup();
		}

		modal.end();
	}
}
