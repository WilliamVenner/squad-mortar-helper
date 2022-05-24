pub(super) fn apply() {
	// colors\[ImGuiCol_(.+?)\]\s*=\s*ImVec4\s*\(([0-9.]+)f?, ([0-9.]+)f?, ([0-9.]+)f?, ([0-9.]+)f?\);
	// $1 = [$2, $3, $4, $5];

	macro_rules! theme {
		(colors: {$($color_var_name:ident = $color:expr;)*} vars: {$($var_name:ident = $var:expr;)*}) => {
			unsafe {
				let style = &mut *imgui::sys::igGetStyle();

				$(style.Colors[imgui::StyleColor::$color_var_name as u32 as usize] = {
					let [r, g, b, a] = $color;
					imgui::sys::ImVec4::new(r, g, b, a)
				};)*

				$(style.$var_name = $var;)*
			}
		}
	}

	theme! {
		colors: {
			Text = [1.0, 1.0, 1.0, 1.0];
			TextDisabled = [0.5, 0.5, 0.5, 1.0];
			WindowBg = [0.06, 0.06, 0.06, 0.94];
			ChildBg = [1.0, 1.0, 1.0, 0.0];
			PopupBg = [0.08, 0.08, 0.08, 0.94];
			Border = [0.43, 0.43, 0.5, 0.5];
			BorderShadow = [0.0, 0.0, 0.0, 0.0];
			FrameBg = [0.2, 0.21, 0.22, 0.54];
			FrameBgHovered = [0.4, 0.4, 0.4, 0.4];
			FrameBgActive = [0.0, 0.47, 1.0, 1.0];
			TitleBg = [0.04, 0.04, 0.04, 1.0];
			TitleBgActive = [0.0, 0.42, 1.0, 1.0];
			TitleBgCollapsed = [0.0, 0.0, 0.0, 0.51];
			MenuBarBg = [0.14, 0.14, 0.14, 1.0];
			ScrollbarBg = [0.02, 0.02, 0.02, 0.53];
			ScrollbarGrab = [0.31, 0.31, 0.31, 1.0];
			ScrollbarGrabHovered = [0.41, 0.41, 0.41, 1.0];
			ScrollbarGrabActive = [0.51, 0.51, 0.51, 1.0];
			CheckMark = [0.94, 0.94, 0.94, 1.0];
			SliderGrab = [0.51, 0.51, 0.51, 1.0];
			SliderGrabActive = [0.86, 0.86, 0.86, 1.0];
			Button = [0.44, 0.44, 0.44, 0.4];
			ButtonHovered = [0.46, 0.47, 0.48, 1.0];
			ButtonActive = [0.42, 0.42, 0.42, 1.0];
			Header = [0.7, 0.7, 0.7, 0.31];
			HeaderHovered = [0.7, 0.7, 0.7, 0.8];
			HeaderActive = [0.48, 0.5, 0.52, 1.0];
			Separator = [0.43, 0.43, 0.5, 0.5];
			SeparatorHovered = [0.72, 0.72, 0.72, 0.78];
			SeparatorActive = [0.51, 0.51, 0.51, 1.0];
			ResizeGrip = [0.91, 0.91, 0.91, 0.25];
			ResizeGripHovered = [0.81, 0.81, 0.81, 0.67];
			ResizeGripActive = [0.46, 0.46, 0.46, 0.95];
			PlotLines = [0.61, 0.61, 0.61, 1.0];
			PlotLinesHovered = [1.0, 0.43, 0.35, 1.0];
			PlotHistogram = [0.73, 0.6, 0.15, 1.0];
			PlotHistogramHovered = [1.0, 0.6, 0.0, 1.0];
			TextSelectedBg = [0.87, 0.87, 0.87, 0.35];
			DragDropTarget = [1.0, 1.0, 0.0, 0.9];
			NavHighlight = [0.6, 0.6, 0.6, 1.0];
			NavWindowingHighlight = [1.0, 1.0, 1.0, 0.7];

			NavWindowingDimBg = [0.0, 0.0, 0.0, 0.5];
			ModalWindowDimBg = [0.0, 0.0, 0.0, 0.5];
		}

		vars: {
			TabRounding = 0.0;
			ChildRounding = 0.0;
			WindowRounding = 0.0;
			PopupRounding = 0.0;
			GrabRounding = 0.0;
		}
	}
}