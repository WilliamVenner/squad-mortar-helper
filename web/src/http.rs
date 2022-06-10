use super::*;

pub fn build_response(ws_port: u16) -> Arc<str> {
	#[cfg(not(debug_assertions))]
	let response = include_str!(concat!(env!("OUT_DIR"), "/web.html"));

	#[cfg(debug_assertions)]
	let response = html::rebuild_html();

	let response = response.replace("{{ WEBSOCKET_PORT }}", &ws_port.to_string());
	let response = format!(
		"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: Closed\r\n\r\n{}",
		response.len(),
		response
	);

	Arc::from(response)
}

pub async fn accept_http_connection(mut stream: TcpStream, http_response: &str) -> Result<(), AnyError> {
	use tokio::io::AsyncWriteExt;

	let addr = stream.peer_addr()?;

	log::info!("HTTP connection opened with: {addr}");

	timeout_barrier! {
		{
			stream.write_all(http_response.as_bytes()).await?;
			stream.shutdown().await?;
			Ok::<_, AnyError>(())
		},

		Duration::from_secs(10) => {
			log::warn!("HTTP connection timeout with {addr}");
			Ok(())
		}
	}?;

	Ok(())
}