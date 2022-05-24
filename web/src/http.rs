use super::*;

pub async fn accept_http_connection(mut stream: TcpStream, ws_port: &str) -> Result<(), AnyError> {
	use tokio::io::AsyncWriteExt;

	let addr = stream.peer_addr()?;

	log::info!("HTTP connection opened with: {}", addr);

	#[cfg(not(debug_assertions))]
	let response = include_str!(concat!(env!("OUT_DIR"), "/web.html"));

	#[cfg(debug_assertions)]
	let response = html::rebuild_html();

	let response = response.replacen("{{ WEBSOCKET_PORT }}", ws_port, 1);
	let response = format!(
		"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: Closed\r\n\r\n{}",
		response.len(),
		response
	);

	timeout_barrier! {
		{
			stream.write_all(response.as_bytes()).await?;
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