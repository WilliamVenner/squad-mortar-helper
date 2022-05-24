var WS_EVENT_MAP_FRAME = 1;
var WS_EVENT_MARKERS = 2;
var WS_EVENT_UPDATE_STATE = 3;
var WS_EVENT_HEIGHTMAP = 4;

var WS_INTERACTION_ADD_CUSTOM_MARKER = 1;
var WS_INTERACTION_DELETE_CUSTOM_MARKER = 2;

var ws;

function ws_interaction(event, size_fn, data_fn) {
	if (ws && ws.readyState == WebSocket.OPEN) {
		var data = new DataView(new ArrayBuffer(2 + size_fn()));
		data.setUint16(0, event, true);
		data_fn(2, data);
		ws.send(data.buffer);
	}
}

function connect() {
	set_status('Connecting...');

	ws = new WebSocket('ws://' + location.hostname + ':' + ws_port);
	ws.binaryType = 'arraybuffer';

	ws.addEventListener('open', function() {
		set_status();
	});

	ws.addEventListener('error', function(e) {
		console.error(e);

		var code = e.code;
		if (code == null) {
			code = 'Unknown';
		}

		var reason = e.reason;
		if (reason) {
			reason = ' (' + reason + ')';
		}

		set_status('Error: ' + code + reason);
	});

	ws.addEventListener('close', function() {
		set_status('Connection closed');
		setTimeout(connect, 2000);
	});

	ws.addEventListener('message', function(e) {
		var event = new DataView(e.data.slice(0, 2)).getUint16(0, true);
		var data = e.data.slice(2);
		map_event(event, data);
	});
}

connect();