use super::*;

pub(super) fn menu_bar(ui: &Ui) {
	if let Some(about) = ui.begin_menu("About") {
		if imgui::MenuItem::new("GitHub").build(ui) {
			open::that("https://github.com/WilliamVenner/squad-mortar-helper").ok();
		}

		imgui::MenuItem::new(concat!("v", env!("CARGO_PKG_VERSION"), " (", build_time::build_time_local!("%d %b %Y"), ")")).build(ui);

		if imgui::MenuItem::new("By William Venner (Billy)").build(ui) {
			open::that("https://github.com/WilliamVenner").ok();
		}

		about.end();
	}
}

pub(super) fn render_star_pls(state: &mut UiState, ui: &Ui) {
	if !state.star_modal {
		state.star_modal = true;

		// Show them the popup once they've opened SMH three times
		let times_opened = SETTINGS.github_star_modal();
		match times_opened {
			0..=1 => {},
			2 => ui.open_popup("Squad Mortar Helper"),
			3.. => return
		}

		SETTINGS.set_github_star_modal(times_opened + 1);
	}

	if let Some(modal) = imgui::PopupModal::new("Squad Mortar Helper").always_auto_resize(false).resizable(false).begin_popup(ui) {
		ui.text("If you have a GitHub account, please consider dropping a star\non the GitHub repository if my work helped you :D");

		ui.spacing();

		if ui.button("Sure") {
			ui.close_current_popup();
			open::that("https://github.com/WilliamVenner/squad-mortar-helper/stargazers").ok();
		}

		ui.same_line();

		if ui.button("Piss off") {
			ui.close_current_popup();
		}

		modal.end();
	}
}