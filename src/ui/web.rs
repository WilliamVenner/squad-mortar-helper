use super::*;

use smh_web::*;

enum WebModal {
	Port(i32),
	Result(Result<Box<str>, AnyError>)
}

#[derive(Default)]
pub struct WebState {
	pub server: Option<WebServer>,
	modal: Option<(bool, WebModal)>
}

pub(super) fn menu_bar(state: &mut UIState, ui: &Ui) {
	if let Some(web) = ui.begin_menu("Web") {
		if let Some(server) = &state.web.server {
			let num_clients = server.num_clients();
			if num_clients == 1 {
				imgui::MenuItem::new("1 client connected").build(ui);
			} else {
				imgui::MenuItem::new(&ui_format!(state, "{} clients connected", num_clients)).build(ui);
			}

			if imgui::MenuItem::new(&server.addr).build(ui) {
				ui.set_clipboard_text(&server.addr);
			}
		}

		if state.web.server.is_some() {
			if imgui::MenuItem::new("Stop Server").build(ui) {
				state.web.server.take();
				state.web.modal = None;
			}
		} else if imgui::MenuItem::new("Start Server").build(ui) {
			state.web.modal = Some((false, WebModal::Port(0)));
		}

		web.end();
	}
}

pub(super) fn render_popup(state: &mut UIState, ui: &Ui) {
	if let Some((ref mut opened, ref mut result)) = state.web.modal {
		if !*opened {
			ui.open_popup(match result {
				WebModal::Result(_) => "Web Server",
				WebModal::Port(_) => "Web Server Port"
			});
			*opened = true;
		}

		match result {
			WebModal::Result(ref result) => if let Some(modal) = imgui::PopupModal::new("Web Server").resizable(false).begin_popup(ui) {
				match result {
					Ok(addr) => {
						ui.text(ui_format!(state, "Server started! To access it, navigate to {} in your browser!", addr));
						ui.spacing();

						if ui.button("Copy") {
							ui.set_clipboard_text(addr);
						}

						ui.same_line();

						if ui.button("OK") {
							ui.close_current_popup();
							state.web.modal = None;
						}
					},

					Err(err) => {
						ui.text(ui_format!(state, "Error starting server: {err}\n{}", err.backtrace()));
						ui.spacing();

						if ui.button("Dismiss") {
							ui.close_current_popup();
							state.web.modal = None;
						}
					}
				}

				modal.end();
			},

			WebModal::Port(port) => if let Some(modal) = imgui::PopupModal::new("Web Server Port").resizable(false).begin_popup(ui) {
				ui.text("What port do you want to host the server on? Enter 0 to use any available port.");
				ui.spacing();

				ui.input_int("Port", port).build();

				ui.spacing();

				if ui.button("Start") {
					ui.close_current_popup();

					let event_data = smh_web::EventData {
						map: state.vision.map.clone(),
						computer_vision_markers: state.vision.markers.iter().map(|marker| [marker.p0, marker.p1]).collect::<Box<_>>(),
						custom_markers: Box::from(&*state.draw.custom_markers),
						meters_to_px_ratio: state.vision.meters_to_px_ratio,
						minimap_bounds: state.vision.minimap_bounds,
						heightmap: squadex::heightmaps::get_current().as_deref().map(ToOwned::to_owned),
						heightmap_fit_to_minimap: state.heightmaps.fit_to_minimap
					};

					let port = (*port).max(0).min(u16::MAX as i32) as u16;
					match WebServer::start(port, redraw, event_data) {
						Err(err) => state.web.modal = Some((false, WebModal::Result(Err(err)))),
						Ok(new) => {
							state.web.modal = Some((false, WebModal::Result(Ok(new.addr.clone()))));
							state.web.server = Some(new);
						},
					}
				}

				ui.same_line();

				if ui.button("Cancel") {
					ui.close_current_popup();
					state.web.modal = None;
				}

				modal.end();
			}
		}
	}
}

pub(super) fn handle_interactions(state: &mut UIState) {
	// needed to get around borrow checker...
	// non-lexical lifetimes when???
	if state.web.server.is_none() {
		return;
	}

	while let Some(interaction) = state.web.server.as_mut().sus_unwrap().recv() {
		match interaction {
			Interaction::AddCustomMarker([p0, p1]) => draw::add_marker(state, p0, p1),
			Interaction::DeleteCustomMarker(id) => draw::delete_marker(state, id as usize)
		}
	}
}