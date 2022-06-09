var left_click = 0;
var right_click = 0;
var middle_click = 0;

var map_zoom = 0;
var map_zoom_pos = [0, 0];
var map_pan_pos = [0, 0];
var map_pan_start = null;

var drag_debounce = false;
var measure_start = null;
var drag_start = null;

var PAN_ACCELERATION = 2;
var MAX_ZOOM = 4;
var ZOOM_LEVELS = 10;
var MOUSE_DRAG_THRESHOLD = 6 ** 2;

function is_line_long_enough(p0, p1) {
	return ((p0[0] - p1[0]) ** 2) + ((p0[1] - p1[1]) ** 2) >= MOUSE_DRAG_THRESHOLD;
}

function is_point_on_line(a, b, c, tolerance) {
	var cross_product = (c[1] - a[1]) * (b[0] - a[0]) - (c[0] - a[0]) * (b[1] - a[1]);
	if (Math.abs(cross_product) > tolerance) {
		return false;
	}

	var dot_product = (c[0] - a[0]) * (b[0] - a[0]) + (c[1] - a[1]) * (b[1] - a[1]);
	if (dot_product < 0.0) {
		return false;
	}

	var length = (b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]);
	if (dot_product > length) {
		return false;
	}

	return true;
}

function apply_zoom_pan() {
	cached_map_viewport = null;

	var map_viewport = get_map_viewport();

	var transform = 'translate(' + map_viewport.top_left[0] + 'px, ' + map_viewport.top_left[1] + 'px) scale(' + map_viewport.scale_factor_w + ', ' + map_viewport.scale_factor_h + ')';
	draw_canvas.style.transform = transform;
	map_canvas.style.transform = transform;
	overlay_canvas.style.transform = transform;

	if (map_zoom !== 0) {
		ui_container.classList.add('zoomed');
	} else {
		ui_container.classList.remove('zoomed');
	}
}
window.addEventListener('resize', apply_zoom_pan);
apply_zoom_pan();

ui_container.addEventListener('mousedown', function(e) {
	e.preventDefault();
	e.stopPropagation();

	switch (e.button) {
		case 0:
			left_click++;
			break;

		case 2:
			right_click++;
			break;

		case 1:
			middle_click++;
			break;
	}

	return ui_container.dispatchEvent(new MouseEvent('mousemove', e));
});

ui_container.addEventListener('mouseup', function(e) {
	e.preventDefault();
	e.stopPropagation();

	switch (e.button) {
		case 0:
			left_click--;
			break;

		case 2:
			right_click--;
			break;

		case 1:
			middle_click--;
			break;
	}

	return ui_container.dispatchEvent(new MouseEvent('mousemove', e));
});

ui_container.addEventListener('mousewheel', function(e) {
	e.preventDefault();
	e.stopPropagation();

	if (-e.deltaY > 0.0) {
		if (map_zoom < ZOOM_LEVELS) {
			var from_zero = map_zoom === 0;

			map_zoom += 1;

			var mouse_pos_f = [Math.min(e.clientX / window.innerWidth, 1.0), Math.min(e.clientY / window.innerHeight, 1.0)];
			if (from_zero) {
				map_zoom_pos = mouse_pos_f;
			} else {
				map_zoom_pos = [
					(map_zoom_pos[0] + mouse_pos_f[0]) / 2,
					(map_zoom_pos[1] + mouse_pos_f[1]) / 2
				];
			}
		}
	} else if (map_zoom > 0) {
		map_zoom -= 1;

		if (map_zoom === 0) {
			map_pan_start = null;
			map_pan_pos = [0, 0];
		}
	}

	apply_zoom_pan();
});

function draw_mouse_ctl(e) {
	if (drag_debounce) {
		if (left_click <= 0 && right_click <= 0) {
			drag_debounce = false;
		}
		return;
	}

	var mouse_pos = inverse_map_xy([e.clientX, e.clientY]);
	if (drag_start === null) {
		if (measure_start !== null) {
			if (right_click <= 0) {
				measure_start = null;
				drag_debounce = true;
			}
		} else if (right_click > 0) {
			measure_start = mouse_pos;
		} else if (left_click > 0) {
			drag_start = mouse_pos;
		}
	} else if (left_click <= 0) {
		if (is_line_long_enough(drag_start, mouse_pos)) {
			ws_interaction(
				WS_INTERACTION_ADD_CUSTOM_MARKER,
				function() {
					return 4 * 4;
				},
				function(offset, data) {
					data.setFloat32(offset, drag_start[0], true);
					data.setFloat32(offset += 4, drag_start[1], true);
					data.setFloat32(offset += 4, mouse_pos[0], true);
					data.setFloat32(offset += 4, mouse_pos[1], true);
				}
			);
		}
		drag_start = null;
	}
}

function zoom_pan_mouse_ctl(e) {
	// mouse wheel
	if (map_zoom !== 0 && middle_click > 0) {
		if (map_pan_start !== null) {
			var delta = [e.clientX - map_pan_start[0], e.clientY - map_pan_start[1]];
			map_pan_pos[0] += delta[0] * PAN_ACCELERATION;
			map_pan_pos[1] += delta[1] * PAN_ACCELERATION;
		}
		map_pan_start = [e.clientX, e.clientY];
		apply_zoom_pan();
	} else if (map_pan_start !== null) {
		map_pan_start = null;
		apply_zoom_pan();
	}
}

function delete_mouse_ctl(e) {
	if (drag_start !== null) {
		return;
	}

	var has_custom_markers = false;
	for (var id in custom_markers) {
		if (custom_markers.hasOwnProperty(id)) {
			has_custom_markers = true;
			break;
		}
	}
	if (has_custom_markers) {
		var tolerance = (Math.sqrt(window.innerWidth * window.innerHeight) / 554.0) * 2000.0;

		var hovered_marker = null;
		for (var i = 0; i < custom_markers.length; i++) {
			var custom_marker = custom_markers[i];
			var hovered = is_point_on_line(
				translate_map_xy([custom_marker.p0x, custom_marker.p0y]),
				translate_map_xy([custom_marker.p1x, custom_marker.p1y]),
				[e.clientX, e.clientY],
				tolerance
			);
			if (hovered) {
				hovered_marker = i;
				break;
			}
		}

		if (hovered_marker !== null) {
			if (left_click > 0) {
				drag_debounce = true;
				ws_interaction(
					WS_INTERACTION_DELETE_CUSTOM_MARKER,
					function() {
						return 4;
					},
					function(offset, data) {
						data.setUint32(offset, hovered_marker, true);
					}
				);
			}
			document.body.style.cursor = 'pointer';
		} else {
			document.body.style.cursor = null;
		}
	}
}

ui_container.addEventListener('mousemove', function(e) {
	delete_mouse_ctl(e);
	draw_mouse_ctl(e);
	zoom_pan_mouse_ctl(e);
	draw_ctl_markers(e);
});

window.addEventListener('keydown', function(e) {
	if ((drag_start !== null || measure_start !== null) && e.key === 'Escape') {
		drag_start = null;
		measure_start = null;
		drag_debounce = true;

		e.preventDefault();
		e.stopPropagation();
	}
});

document.addEventListener('mouseout', function(e) {
    e = e ? e : window.event;
    var from = e.relatedTarget || e.toElement;
    if (!from || from.nodeName == "HTML") {
		drag_start = null;
		measure_start = null;
		left_click = 0;
		right_click = 0;
		middle_click = 0;
		drag_debounce = false;
		draw_ctl_markers(e);
	}
});