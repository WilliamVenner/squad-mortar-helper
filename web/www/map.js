var ui_container = document.getElementById('ui');

var draw_canvas = document.getElementById('draw');
var draw = draw_canvas.getContext('2d');

var map_canvas = document.getElementById('map');
var overlay_canvas = document.getElementById('overlay');

var map = map_canvas.getContext('2d');
var overlay = overlay_canvas.getContext('2d');

var meters_to_px_ratio = null;
var minimap_viewport = null;
var heightmap = null;

var CUSTOM_MARKER_COLOR = [255, 0.0, 255];
var MEASURE_MARKER_COLOR = [255, 0.0, 0.0];

var set_status;
{
	var status_node = document.getElementById('status');
	var status_inner = status_node.childNodes[0];
	set_status = function(text) {
		if (text == null) {
			status_inner.textContent = text;
			status_node.className = '';
		} else {
			status_inner.textContent = text;
			status_node.className = 'active';
		}
	};
};

var cached_map_viewport = null;
function get_map_viewport() {
	if (cached_map_viewport !== null) {
		return cached_map_viewport;
	}

	var map_w = draw_canvas.width;
	var map_h = draw_canvas.height;
	var window_w = window.innerWidth;
	var window_h = window.innerHeight;

	var map_aspect_ratio = map_w / map_h;
	var window_aspect_ratio = window_w / window_h;

	var size;
	if (window_aspect_ratio > map_aspect_ratio) {
		size = [window_h * map_aspect_ratio, window_h];
	} else {
		var map_aspect_ratio = map_h / map_w;
		size = [window_w, window_w * map_aspect_ratio];
	}

	var top_left = [(window_w - size[0]) / 2., (window_h - size[1]) / 2.];
	if (map_zoom !== 0) {
		var zoom_amount = Math.min(map_zoom / ZOOM_LEVELS, 1.0) * MAX_ZOOM;

		top_left[0] -= map_zoom_pos[0] * size[0] * zoom_amount;
		top_left[1] -= map_zoom_pos[1] * size[1] * zoom_amount;

		// Pan
		top_left[0] += map_pan_pos[0] * (size[0] / map_w);
		top_left[1] += map_pan_pos[1] * (size[1] / map_h);

		zoom_amount += 1.0;

		size[0] *= zoom_amount;
		size[1] *= zoom_amount;
	}

	cached_map_viewport = {
		scale_factor_w: size[0] / map_w,
		scale_factor_h: size[1] / map_h,
		top_left
	};

	return cached_map_viewport;
}

function translate_map_x(x) {
	var map_viewport = get_map_viewport();
	return (x * map_viewport.scale_factor_w) + map_viewport.top_left[0];
}

function translate_map_y(y) {
	var map_viewport = get_map_viewport();
	return (y * map_viewport.scale_factor_h) + map_viewport.top_left[1];
}

function translate_map_xy(xy) {
	var map_viewport = get_map_viewport();
	return [
		(xy[0] * map_viewport.scale_factor_w) + map_viewport.top_left[0],
		(xy[1] * map_viewport.scale_factor_h) + map_viewport.top_left[1]
	];
}

function inverse_map_xy(xy) {
	var map_viewport = get_map_viewport();
	return [
		(xy[0] - map_viewport.top_left[0]) / map_viewport.scale_factor_w,
		(xy[1] - map_viewport.top_left[1]) / map_viewport.scale_factor_h
	];
}

function calc_alt_delta(p0, p1) {
	if (!heightmap || !minimap_viewport) {
		return null;
	}

	var map_viewport = get_map_viewport();

	var p0 = translate_map_xy(p0);
	var p1 = translate_map_xy(p1);

	var hm_scale_factor_w = (minimap_viewport.right - minimap_viewport.left) / (heightmap.width + heightmap.offset[0]);
	var hm_scale_factor_h = (minimap_viewport.bottom - minimap_viewport.top) / (heightmap.height + heightmap.offset[1]);
	var offset = [heightmap.offset[0] * hm_scale_factor_w * map_viewport.scale_factor_w, heightmap.offset[1] * hm_scale_factor_h * map_viewport.scale_factor_h];

	var fitted_minimap_viewport = {
		left: translate_map_x(minimap_viewport.left) + offset[0],
		top: translate_map_y(minimap_viewport.top) + offset[1],
		right: translate_map_x(minimap_viewport.right),
		bottom: translate_map_y(minimap_viewport.bottom),
	};

	var p0_xf = (p0[0] - fitted_minimap_viewport.left) / (fitted_minimap_viewport.right - fitted_minimap_viewport.left);
	var p0_yf = (p0[1] - fitted_minimap_viewport.top) / (fitted_minimap_viewport.bottom - fitted_minimap_viewport.top);

	var p1_xf = (p1[0] - fitted_minimap_viewport.left) / (fitted_minimap_viewport.right - fitted_minimap_viewport.left);
	var p1_yf = (p1[1] - fitted_minimap_viewport.top) / (fitted_minimap_viewport.bottom - fitted_minimap_viewport.top);

	var p0_x = p0_xf * heightmap.width;
	var p0_y = p0_yf * heightmap.height;
	var p1_x = p1_xf * heightmap.width;
	var p1_y = p1_yf * heightmap.height;

	// The heightmap can be used to calculate a more accurate length than eyeballing the map scales
	var meters = Math.sqrt(((p0_x - p1_x) ** 2) + ((p0_y - p1_y) ** 2));

	var p0_x = Math.round(p0_x);
	var p0_y = Math.round(p0_y);
	var p1_x = Math.round(p1_x);
	var p1_y = Math.round(p1_y);

	if (p0_x >= 0 && p0_y >= 0 && p1_x >= 0 && p1_y >= 0 && p0_x < heightmap.width && p0_y < heightmap.height && p1_x < heightmap.width && p1_y < heightmap.height) {
		var p0 = heightmap.data[p0_y * heightmap.width + p0_x];
		var p1 = heightmap.data[p1_y * heightmap.width + p1_x];

		p0 = (p0 / 65535) * (heightmap.scale / 0.1953125);
		p1 = (p1 / 65535) * (heightmap.scale / 0.1953125);

		return [Math.round(p1 - p0), meters];
	} else {
		return null;
	}
}

var computer_vision_markers = [];
var custom_markers = [];
function draw_marker(ctx, marker, color) {
	ctx.lineWidth = 2;
	ctx.strokeStyle = 'rgb(' + color[0] + ',' + color[1] + ',' + color[2] + ')';
	ctx.beginPath();
	ctx.moveTo(marker.p0x, marker.p0y);
	ctx.lineTo(marker.p1x, marker.p1y);
	ctx.stroke();

	ctx.font = '600 1.5em \'Inter\', sans-serif';
	ctx.textAlign = 'center';
	ctx.textBaseline = 'top';
	ctx.fillStyle = 'rgb(' + color[0] + ',' + color[1] + ',' + color[2] + ')';

	var heightmap_data = calc_alt_delta([marker.p0x, marker.p0y], [marker.p1x, marker.p1y]);
	var alt_delta = null;
	var meters;

	if (heightmap_data) {
		alt_delta = heightmap_data[0];
		meters = heightmap_data[1];
	} else if (meters_to_px_ratio !== null) {
		var dist = Math.sqrt(((marker.p0x - marker.p1x) ** 2) + ((marker.p0y - marker.p1y) ** 2));
		meters = meters_to_px_ratio * dist;
	} else {
		return;
	}

	var angle = Math.atan2(marker.p0y - marker.p1y, marker.p0x - marker.p1x);

	var bearing_fwd = angle * 180 / Math.PI;
	if (bearing_fwd > 0) {
		bearing_fwd -= 90;
		if (bearing_fwd < 0) {
			bearing_fwd += 360;
		}
	} else {
		bearing_fwd += 270;
	}
	var bearing_bck = (bearing_fwd + 180) % 360;

	var text_angle = angle;
	if (text_angle >= Math.PI / 2) {
		text_angle -= Math.PI;
	} else if (text_angle <= -Math.PI / 2) {
		text_angle += Math.PI;
	}

	var midpoint = [(marker.p0x + marker.p1x) / 2, (marker.p0y + marker.p1y) / 2];

	ctx.save();
	ctx.translate(midpoint[0], midpoint[1]);
	ctx.rotate(text_angle);

	var meters_text = Math.round(meters) + 'm';
	var meters_text_height = ctx.measureText(meters_text).actualBoundingBoxDescent;

	var line_height = meters_text_height * 0.35;

	if (isNaN(alt_delta) || alt_delta !== null) {
		var alt_delta_text = '±' + Math.round(Math.abs(alt_delta)) + 'm alt';
		var alt_delta_text_height = ctx.measureText(alt_delta_text).actualBoundingBoxDescent;

		var alt_delta_fwd = alt_delta;
		var alt_delta_bck = -alt_delta;

		var flip = angle >= -(Math.PI / 2) && angle <= Math.PI / 2;

		var fwd_text;
		var bck_text;

		{
			var alt_delta;
			var bearing;
			if (flip) {
				alt_delta = alt_delta_fwd;
				bearing = bearing_fwd;
			} else {
				alt_delta = alt_delta_bck;
				bearing = bearing_bck;
			}

			var milliradians = milliradians_from_meters(meters, alt_delta);

			var milliradians_text = isNaN(milliradians) ? '<- RANGE!' : ('<- ' + Math.round(milliradians) + ' mil');
			var bearing_text = Math.round(bearing) + '°';

			fwd_text = [
				milliradians_text,
				bearing_text
			];

			if (!window.RELEASE) {
				fwd_text.push(Math.round(alt_delta) + 'm alt');
			}
		}

		{
			var alt_delta;
			var bearing;
			if (flip) {
				alt_delta = alt_delta_bck;
				bearing = bearing_bck;
			} else {
				alt_delta = alt_delta_fwd;
				bearing = bearing_fwd;
			}

			var milliradians = milliradians_from_meters(meters, alt_delta);

			var milliradians_text = isNaN(milliradians) ? 'RANGE! ->' : (Math.round(milliradians) + ' mil ->');
			var bearing_text = Math.round(bearing) + '°';

			bck_text = [
				milliradians_text,
				bearing_text
			];

			if (!window.RELEASE) {
				bck_text.push(Math.round(alt_delta) + 'm alt');
			}
		}

		var y_base = line_height;
		ctx.fillText(meters_text, 0, y_base);
		y_base += line_height + meters_text_height;
		ctx.fillText(alt_delta_text, 0, y_base);
		y_base += line_height + alt_delta_text_height;

		ctx.textAlign = 'right';
		var y = y_base;
		for (var i = 0; i < fwd_text.length; i++) {
			ctx.fillText(fwd_text[i], -10, y);
			y += line_height + ctx.measureText(fwd_text[i]).actualBoundingBoxDescent;
		}

		ctx.textAlign = 'left';
		var y = y_base;
		for (var i = 0; i < bck_text.length; i++) {
			ctx.fillText(bck_text[i], 10, y);
			y += line_height + ctx.measureText(bck_text[i]).actualBoundingBoxDescent;
		}
	} else {
		var milliradians = milliradians_from_meters(meters, alt_delta);
		var milliradians_text = isNaN(milliradians) ? 'RANGE!' : (Math.round(milliradians) + ' mils');
		var bearing_text;
		var bearing_bck_text;
		if (angle >= -(Math.PI / 2) && angle <= Math.PI / 2) {
			bearing_text = '-> ' + Math.round(bearing_bck) + '°';
			bearing_bck_text = '<- ' + Math.round(bearing_fwd) + '°';
		} else {
			bearing_text = '-> ' + Math.round(bearing_fwd) + '°';
			bearing_bck_text = '<- ' + Math.round(bearing_bck) + '°';
		}

		var milliradians_text_height = ctx.measureText(milliradians_text).actualBoundingBoxDescent;
		var bearing_text_height = ctx.measureText(bearing_text).actualBoundingBoxDescent;

		var i = line_height;
		ctx.fillText(meters_text, 0, i);
		i += line_height + meters_text_height;
		ctx.fillText(milliradians_text, 0, i);
		i += line_height + milliradians_text_height;
		ctx.fillText(bearing_text, 0, i);
		i += line_height + bearing_text_height;
		ctx.fillText(bearing_bck_text, 0, i);
	}

	ctx.restore();
}

function draw_markers() {
	overlay.clearRect(0, 0, overlay_canvas.width, overlay_canvas.height);

	for (var i = 0; i < computer_vision_markers.length; i++) {
		var f = (i + 1) / computer_vision_markers.length;
		var color = [(1 - f) * 255, f * 255, 0];
		draw_marker(overlay, computer_vision_markers[i], color);
	}
	for (var i = 0; i < custom_markers.length; i++) {
		draw_marker(overlay, custom_markers[i], CUSTOM_MARKER_COLOR);
	}
}

function draw_ctl_markers(e) {
	draw.clearRect(0, 0, draw_canvas.width, draw_canvas.height);

	var mouse_pos = [e.clientX, e.clientY];

	if (drag_start !== null) {
		var mouse_pos = inverse_map_xy(mouse_pos);
		if (is_line_long_enough(drag_start, mouse_pos)) {
			draw_marker(
				draw,
				{
					p0x: drag_start[0],
					p0y: drag_start[1],
					p1x: mouse_pos[0],
					p1y: mouse_pos[1]
				},
				CUSTOM_MARKER_COLOR
			);
		}
	}

	if (measure_start !== null) {
		var mouse_pos = inverse_map_xy(mouse_pos);
		if (is_line_long_enough(measure_start, mouse_pos)) {
			draw_marker(
				draw,
				{
					p0x: measure_start[0],
					p0y: measure_start[1],
					p1x: mouse_pos[0],
					p1y: mouse_pos[1],
				},
				MEASURE_MARKER_COLOR
			);
		}
	}
}

function map_event(event, data) {
	switch (event) {
		case WS_EVENT_MAP_FRAME: // Map
			var width;
			var height;
			{
				var dimensions = new DataView(data.slice(0, 8))
				width = dimensions.getUint32(0, true);
				height = dimensions.getUint32(4, true);
			}

			map_canvas.width = width;
			map_canvas.height = height;
			overlay_canvas.width = width;
			overlay_canvas.height = height;
			draw_canvas.width = width;
			draw_canvas.height = height;
			apply_zoom_pan();

			var image = map.createImageData(width, height);
			image.data.set(new Uint8Array(data.slice(8)));
			map.putImageData(image, 0, 0, 0, 0, width, height);

			computer_vision_markers = [];
			draw_markers();

			break;

		case WS_EVENT_MARKERS: // Markers
			var markers = new DataView(data);
			var offset = 0;

			var out;
			if (markers.getUint8(offset) === 1) {
				custom_markers = [];
				out = custom_markers;
			} else {
				computer_vision_markers = [];
				out = computer_vision_markers;
			}

			var len = markers.getUint32(offset += 1, true);

			for (var i = 0; i < len; i++) {
				out.push({
					p0x: markers.getFloat32(offset += 4, true),
					p0y: markers.getFloat32(offset += 4, true),
					p1x: markers.getFloat32(offset += 4, true),
					p1y: markers.getFloat32(offset += 4, true),
				});
			}

			draw_markers();
			break;

		case WS_EVENT_UPDATE_STATE: // UpdateState
			var data = new DataView(data);

			meters_to_px_ratio = data.getFloat64(0, true);
			if (meters_to_px_ratio === 0) {
				meters_to_px_ratio = null;
			}

			if (data.getUint8(8) === 1) {
				minimap_viewport = {
					left: data.getUint32(9, true),
					right: data.getUint32(13, true),
					top: data.getUint32(17, true),
					bottom: data.getUint32(21, true)
				};
			} else {
				minimap_viewport = null;
			}

			draw_markers();
			break;

		case WS_EVENT_HEIGHTMAP: // Heightmap
			var data = new DataView(data);
			var offset = 0;
			if (data.getUint8(0) === 1) {
				// HACK! Fixes RangeError "start offset of Uint16Array should be a multiple of 2"
				// javascript, why? why is alignment even an issue if you memcpy everything anyway?
				offset += 1;

				heightmap = {
					width: data.getUint32(offset += 1, true),
					height: data.getUint32(offset += 4, true),
					offset: [data.getInt32(offset += 4, true), data.getInt32(offset += 4, true)],
					scale: data.getFloat32(offset += 4, true),
					data: new Uint16Array(data.buffer, offset += 4, (data.byteLength - offset) / 2)
				};
			} else {
				heightmap = null;
			}

			draw_markers();
			break;

		default:
			console.error('Unknown event: ' + event);
			console.error(data);
	}
}