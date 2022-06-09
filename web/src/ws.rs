use super::*;

fn is_fatal_err(err: &tokio_tungstenite::tungstenite::Error) -> bool {
	match err {
		tokio_tungstenite::tungstenite::Error::AlreadyClosed | tokio_tungstenite::tungstenite::Error::ConnectionClosed => true,
		tokio_tungstenite::tungstenite::Error::Io(err)
			if matches!(
				err.kind(),
				std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
			) =>
		{
			true
		}
		_ => false,
	}
}

pub(super) async fn accept_ws_connection(
	stream: TcpStream,
	interaction_tx: (tokio::sync::mpsc::Sender<Interaction>, fn()),
	event_rx: async_channel::Receiver<Vec<u8>>,
	event_data: EventData,
) -> Result<SocketAddr, AnyError> {
	let addr = stream.peer_addr()?;

	log::info!("WebSocket Connection opened with: {}", addr);

	let stream = tokio_tungstenite::accept_async(stream).await?;
	let (mut w, mut r) = stream.split();

	// Send the initial event data to the client
	{
		use tokio_tungstenite::tungstenite::Message::Binary;

		if event_data.map.dimensions() != (0, 0) {
			w.send(Binary(Event::Map { map: event_data.map }.serialize())).await?;
		}

		if event_data.meters_to_px_ratio.is_some() || event_data.minimap_bounds.is_some() {
			w.send(Binary(Event::UpdateState { meters_to_px_ratio: event_data.meters_to_px_ratio, minimap_bounds: event_data.minimap_bounds }.serialize())).await?;
		}

		if !event_data.computer_vision_markers.is_empty() {
			w.send(Binary(Event::Markers { custom: false, markers: event_data.computer_vision_markers }.serialize())).await?;
		}

		if !event_data.custom_markers.is_empty() {
			w.send(Binary(Event::Markers { custom: true, markers: event_data.custom_markers }.serialize())).await?;
		}

		if let Some(ref heightmap) = event_data.heightmap {
			w.send(Binary(Event::Heightmap { heightmap: Some(heightmap.clone()) }.serialize())).await?;
		}

		w.send(Binary(Event::HeightmapFitToMinimap { fit_to_minimap: event_data.heightmap_fit_to_minimap }.serialize())).await?;
	}

	loop {
		tokio::select! {
			msg = r.next() => match msg {
				Some(Ok(msg)) => if matches!(msg, tokio_tungstenite::tungstenite::Message::Text(_) | tokio_tungstenite::tungstenite::Message::Binary(_)) {
					let msg = msg.into_data();
					let msg = match Interaction::deserialize(&msg) {
						Some(msg) => msg,
						None => {
							log::warn!("Unknown interaction received from {addr}");
							continue;
						}
					};
					let interaction_tx = interaction_tx.clone();
					tokio::task::spawn(async move {
						interaction_tx.0.send(msg).await.ok();
						interaction_tx.1();
					});
				},

				Some(Err(err)) => {
					if is_fatal_err(&err) {
						break;
					}
					match err {
						tokio_tungstenite::tungstenite::Error::Io(err) => log::warn!("Error receiving interaction from {addr}: ({:?}) {err}", err.kind()),
						_ => log::warn!("Error receiving interaction from {addr}: {err}")
					}
				},

				None => break
			},

			event = event_rx.recv() => match event {
				Ok(event) => {
					let res = timeout_barrier! {
						w.send(tokio_tungstenite::tungstenite::Message::Binary(event)).await,

						Duration::from_secs(10) => {
							log::warn!("WebSocket connection timeout with {addr}");
							break;
						}
					};
					if let Err(err) = res {
						if is_fatal_err(&err) {
							break;
						}
						match err {
							tokio_tungstenite::tungstenite::Error::Io(err) => log::warn!("Error sending event to {addr}: ({:?}) {err}", err.kind()),
							_ => log::warn!("Error sending event to {addr}: {err}")
						}
					}
				},

				Err(_) => break
			}
		}
	}

	Ok(addr)
}
