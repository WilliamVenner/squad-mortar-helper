use futures_util::{SinkExt, StreamExt};
use image::EncodableLayout;
use smh_heightmap_ripper::Heightmap;
use smh_util::{anyhow, async_channel, image, log, FromBytesSlice, Rect};
use std::{
	net::{Ipv4Addr, SocketAddr, SocketAddrV4},
	sync::Arc,
	thread::JoinHandle,
	time::Duration,
};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

// TODO use byteorder

type AnyError = anyhow::Error;

/// Wraps some async code that should time out after a certain amount of time.
macro_rules! timeout_barrier {
	{$code:expr, $duration:expr => $timeout:expr} => {
		match tokio::time::timeout($duration, async { $code }).await {
			Ok(res) => res,
			Err(_) => $timeout
		}
	};
}

#[cfg(debug_assertions)]
mod html;

mod http;
mod ws;

pub enum Interaction {
	AddCustomMarker([[f32; 2]; 2]),
	DeleteCustomMarker(u32),
}
impl Interaction {
	pub fn deserialize(data: &[u8]) -> Option<Self> {
		if data.len() < 2 {
			return None;
		}

		let interaction = u16::from_le_bytes_slice(&data[0..2]);
		let data = &data[2..];

		match interaction {
			1 => {
				if data.len() != core::mem::size_of::<[[f32; 2]; 2]>() {
					log::warn!("Invalid custom marker data length");
					return None;
				}

				Some(Interaction::AddCustomMarker([
					[f32::from_le_bytes_slice(&data[0..4]), f32::from_le_bytes_slice(&data[4..8])],
					[f32::from_le_bytes_slice(&data[8..12]), f32::from_le_bytes_slice(&data[12..16])],
				]))
			}
			2 => {
				if data.len() != core::mem::size_of::<u32>() {
					log::warn!("Invalid custom marker data length");
					return None;
				}

				Some(Interaction::DeleteCustomMarker(u32::from_le_bytes_slice(&data[0..4])))
			}
			_ => {
				log::warn!("Unknown interaction type: {interaction}");
				None
			}
		}
	}
}

macro_rules! events {
	($buf:ident, $($name:ident$({$($field:ident: $ty:ty),*})? => { size => $size:expr, serialize => $serialize:expr }),*) => {
		pub enum Event {
			#[doc(hidden)]
			#[allow(dead_code)]
			Zero,

			$($name$({$($field: $ty),*})?),*
		}
		impl Event {
			#[inline]
			pub fn serialize_inner(&self) -> Result<Vec<u8>, std::io::Error> {
				use std::io::Write;

				let id: u16 = {
					#[repr(u16)]
					enum EventId {
						#[doc(hidden)]
						#[allow(dead_code)]
						Zero = 0,

						$($name),*
					}
					match self {
						Self::Zero => unsafe { core::hint::unreachable_unchecked() },

						$(Self::$name { .. } => EventId::$name as u16),*
					}
				};

				match self {
					Self::Zero => unsafe { core::hint::unreachable_unchecked() },

					$(Self::$name$({$($field),*})? => {
						let mut $buf: Vec<u8> = Vec::with_capacity($size + 2);

						$buf.write_all(&id.to_le_bytes())?;

						$serialize;

						debug_assert_eq!($buf.capacity(), $buf.len(), "Miscalculation in buffer size for event {}", stringify!($name));

						Ok($buf)
					}),*
				}
			}

			pub fn serialize(&self) -> Vec<u8> {
				self.serialize_inner().unwrap()
			}
		}
	}
}
events! {
	buf,

	Map { map: Arc<image::RgbaImage> } => {
		size => {
			(core::mem::size_of::<u32>() * 2) + (map.width() as usize * map.height() as usize * 4)
		},

		serialize => {
			buf.write_all(&u32::to_le_bytes(map.width()))?;
			buf.write_all(&u32::to_le_bytes(map.height()))?;
			buf.write_all(map.as_bytes())?;
		}
	},

	Markers { markers: Box<[[[f32; 2]; 2]]>, custom: bool } => {
		size => {
			((core::mem::size_of::<[f32; 2]>() * 2) * markers.len()) + core::mem::size_of::<u32>() + 1
		},

		serialize => {
			buf.write_all(&[*custom as u8])?;
			buf.write_all(&u32::to_le_bytes(markers.len() as u32))?;
			markers.iter().flat_map(|[p0, p1]| p0.iter().chain(p1).copied()).try_for_each(|xy| buf.write_all(&f32::to_le_bytes(xy)))?;
		}
	},

	UpdateState { meters_to_px_ratio: Option<f64>, minimap_bounds: Option<Rect<u32>> } => {
		size => {
			core::mem::size_of::<f64>() +
			if minimap_bounds.is_some() {
				1 + (core::mem::size_of::<u32>() * 4)
			} else {
				1
			}
		},
		serialize => {
			buf.write_all(&f64::to_le_bytes(meters_to_px_ratio.unwrap_or(0.0)))?;

			if let Some(minimap_bounds) = minimap_bounds {
				buf.write_all(&[1])?;
				buf.write_all(&u32::to_le_bytes(minimap_bounds.left))?;
				buf.write_all(&u32::to_le_bytes(minimap_bounds.right))?;
				buf.write_all(&u32::to_le_bytes(minimap_bounds.top))?;
				buf.write_all(&u32::to_le_bytes(minimap_bounds.bottom))?;
			} else {
				buf.write_all(&[0])?;
			}
		}
	},

	Heightmap { heightmap: Option<Heightmap> } => {
		size => {
			if let Some(heightmap) = heightmap {
				1 + 1 + (core::mem::size_of::<u32>() * 2) + core::mem::size_of::<[i32; 2]>() + core::mem::size_of::<f32>() + (heightmap.data.len() * core::mem::size_of::<u16>())
			} else {
				1
			}
		},
		serialize => {
			let _ = 1 + 1; // fixes clippy::unnecessary_operation

			if let Some(heightmap) = heightmap {
				buf.write_all(&[1])?;

				// HACK! Fixes RangeError "start offset of Uint16Array should be a multiple of 2"
				// javascript, why? why is alignment even an issue if you memcpy everything anyway?
				buf.write_all(&[0])?;

				buf.write_all(&u32::to_le_bytes(heightmap.width))?;
				buf.write_all(&u32::to_le_bytes(heightmap.height))?;
				buf.write_all(&i32::to_le_bytes(heightmap.bounds[0][0]))?;
				buf.write_all(&i32::to_le_bytes(heightmap.bounds[0][1]))?;
				buf.write_all(&f32::to_le_bytes(heightmap.scale[2]))?;
				buf.write_all(unsafe { core::slice::from_raw_parts(heightmap.data.as_ptr() as *const u8, heightmap.data.len() * 2) })?;
			} else {
				buf.write_all(&[0])?;
			}
		}
	},

	HeightmapFitToMinimap { fit_to_minimap: bool } => {
		size => 1,
		serialize => {
			buf.write_all(&[*fit_to_minimap as u8])?;
		}
	}
}

#[derive(Default, Clone)]
pub struct EventData {
	pub map: Arc<image::RgbaImage>,
	pub computer_vision_markers: Box<[[[f32; 2]; 2]]>,
	pub custom_markers: Box<[[[f32; 2]; 2]]>,
	pub meters_to_px_ratio: Option<f64>,
	pub minimap_bounds: Option<Rect<u32>>,
	pub heightmap: Option<smh_heightmap_ripper::Heightmap>,
	pub heightmap_fit_to_minimap: bool,
}

pub struct WebServer {
	pub addr: Box<str>,

	threads: Option<(JoinHandle<()>, tokio::runtime::Handle)>,

	shutdown_tx: tokio::sync::mpsc::Sender<()>,

	event_tx: tokio::sync::mpsc::Sender<Arc<Event>>,
	interaction_rx: tokio::sync::mpsc::Receiver<Interaction>,

	num_clients: Arc<()>,
}
impl WebServer {
	pub fn shutdown(self) {}

	pub fn start(port: u16, wake_ui: fn(), event_data: EventData) -> Result<Self, AnyError> {
		let (tx, rx) = tokio::sync::oneshot::channel();

		let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
		let rt_handle = rt.handle().clone();

		let thread = std::thread::spawn(move || {
			let mut tx = Some(tx);
			if let Err(err) = rt.block_on(server(port, wake_ui, &mut tx, event_data)) {
				log::warn!("Server Error: {err}");

				if let Some(tx) = tx {
					tx.send(Err(err)).ok();
				}
			}
		});

		let server = match rx.blocking_recv().map_err(AnyError::from) {
			Ok(Ok(server)) => Ok(server),
			Err(err) | Ok(Err(err)) => Err(err),
		};

		server.map(|mut server| {
			server.threads = Some((thread, rt_handle));
			server
		})
	}

	#[inline]
	pub fn send(&self, event: impl Into<Arc<Event>>) {
		if let Some((_, rt)) = &self.threads {
			let event = event.into();
			let event_tx = self.event_tx.clone();
			rt.spawn(async move {
				event_tx.send(event).await.ok();
			});
		}
	}

	#[inline]
	pub fn recv(&mut self) -> Option<Interaction> {
		self.interaction_rx.try_recv().ok()
	}

	#[inline]
	pub fn num_clients(&self) -> usize {
		Arc::strong_count(&self.num_clients) - 2
	}
}
impl Drop for WebServer {
	fn drop(&mut self) {
		log::info!("Shutting down...");

		self.shutdown_tx.blocking_send(()).ok();

		if let Some((thread, _)) = self.threads.take() {
			thread.join().ok();
		}
	}
}

async fn server(
	port: u16,
	wake_ui: fn(),
	server_tx: &mut Option<tokio::sync::oneshot::Sender<Result<WebServer, AnyError>>>,
	mut event_data: EventData,
) -> Result<(), AnyError> {
	let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
	let (interaction_tx, interaction_rx) = tokio::sync::mpsc::channel(8);
	let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(8);
	let (broadcast_event_tx, broadcast_event_rx) = async_channel::unbounded();

	let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));

	let ws = TcpListener::bind(addr).await?;
	log::info!("WebServer listening on {}", ws.local_addr()?);

	let http = TcpListener::bind(addr).await?;
	log::info!("HTTP server listening on {}", http.local_addr()?);

	let http_addr = http.local_addr()?;
	let http_response = http::build_response(ws.local_addr()?.port());

	let num_clients = Arc::new(());

	server_tx
		.take()
		.unwrap()
		.send(Ok(WebServer {
			num_clients: num_clients.clone(),
			threads: None,
			event_tx,
			interaction_rx,
			shutdown_tx,
			addr: {
				let addr = match UdpSocket::bind("0.0.0.0:0").await.ok() {
					None => None,
					Some(socket) => tokio::time::timeout(Duration::from_secs(2), socket.connect("8.8.8.8:80"))
						.await
						.ok()
						.and_then(Result::ok)
						.and_then(|_| {
							let mut addr = http_addr;
							addr.set_ip(socket.local_addr().ok()?.ip());
							Some(addr)
						})
						.or_else(|| {
							let mut addr = http_addr;
							addr.set_ip(local_ip_address::local_ip().ok()?);
							Some(addr)
						}),
				};

				if let Some(addr) = addr {
					format!("http://{addr}").into_boxed_str()
				} else {
					format!("http://localhost:{port}").into_boxed_str()
				}
			},
		}))
		.ok();

	loop {
		tokio::select! {
			_ = shutdown_rx.recv() => break,

			event = event_rx.recv() => match event {
				Some(event) => {
					match &*event {
						Event::Markers { custom, markers } => {
							if *custom {
								event_data.custom_markers = markers.clone();
							} else {
								event_data.computer_vision_markers = markers.clone();
							}
						},
						Event::Map { map } => event_data.map = map.clone(),
						Event::UpdateState { meters_to_px_ratio, minimap_bounds } => {
							event_data.meters_to_px_ratio = *meters_to_px_ratio;
							event_data.minimap_bounds = *minimap_bounds;
						},
						Event::Heightmap { heightmap } => {
							event_data.heightmap = heightmap.clone();
						},
						Event::HeightmapFitToMinimap { fit_to_minimap } => {
							event_data.heightmap_fit_to_minimap = *fit_to_minimap;
						},

						_ => {}
					}

					let broadcast_event_tx = broadcast_event_tx.clone();
					tokio::spawn(async move {
						let event = event.serialize();
						broadcast_event_tx.send(event).await.ok();
					});
				},

				None => break
			},

			res = http.accept() => match res {
				Err(err) => log::warn!("Error accepting a new HTTP connection: {err}"),

				Ok((stream, _)) => {
					let http_response = http_response.clone();
					tokio::spawn(async move {
						if let Err(err) = http::accept_http_connection(stream, &*http_response).await {
							log::warn!("HTTP Error: {err}");
						}
					});
				}
			},

			res = ws.accept() => match res {
				Err(err) => log::warn!("Error accepting a new WebSocket connection: {err}"),

				Ok((stream, _)) => {
					let num_clients = num_clients.clone();
					let interaction_tx = (interaction_tx.clone(), wake_ui);
					let broadcast_event_rx = broadcast_event_rx.clone();
					let event_data = event_data.clone();
					tokio::spawn(async move {
						match ws::accept_ws_connection(stream, interaction_tx, broadcast_event_rx, event_data).await {
							Ok(addr) => log::info!("WebSocket connection closed with {addr}"),
							Err(err) => log::warn!("WebSocketError: {err}")
						}
						drop(num_clients);
					});
				}
			}
		}
	}

	log::info!("Shut down");

	Ok(())
}
