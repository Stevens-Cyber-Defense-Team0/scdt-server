use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use thiserror::Error;
use crate::models::PortType;
use crate::guest::GuestCode;

const RESPONSE_OK: u8 = 0;

#[derive(Debug, Error)]
pub enum ChalldError {
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("response not OK, recieved code {0} instead")]
	ResponseCode(u8)
}

pub enum StartMode {
	Guest(GuestCode),
	Demo
}

impl StartMode {
	fn guest_code_len(&self) -> usize {
		match self {
			Self::Guest(_) => 4,
			Self::Demo => 0
		}
	}

	fn mode_id(&self) -> u8 {
		match self {
			Self::Guest(_) => 0,
			Self::Demo => 1
		}
	}
}

impl From<GuestCode> for StartMode {
	fn from(guest_code: GuestCode) -> Self {
		Self::Guest(guest_code)
	}
}

/// returns a WireGuard config if used in guest mode, otherwise returns an empty string
pub async fn start_container<M: Into<StartMode>>(image_name: &str, port_mappings: Vec<(u16, u16, PortType)>, start_mode: M) -> Result<String, ChalldError> {
	let start_mode: StartMode = start_mode.into();

	let mut message = Vec::with_capacity(
		1 + // mode
		start_mode.guest_code_len() + // guest code (for auditing, if present)
		1 + // name length
		image_name.len() + // name
		1 + // number of port mappings
		port_mappings.len() * 5 // port mappings
	);
	message.push(start_mode.mode_id());
	if let StartMode::Guest(guest_code) = start_mode {
		message.extend(guest_code.0.to_le_bytes());
	}
	message.push(image_name.len() as u8);
	message.extend(image_name.as_bytes());
	message.push(port_mappings.len() as u8);

	for (from_port, to_port, port_type) in port_mappings {
		message.extend(from_port.to_le_bytes());
		message.extend(to_port.to_le_bytes());
		message.push(match port_type {
			PortType::Tcp => 0,
			PortType::Udp => 1
		});
	}

	let mut conn = UnixStream::connect("/etc/challd/challd.sock").await?;
	conn.write_all(&message).await?;
	let resp = conn.read_u8().await?;

	if resp == RESPONSE_OK {
		if matches!(start_mode, StartMode::Guest(_)) {
			let len = conn.read_u16_le().await?;
			let mut buf = vec![0; len as usize];
			conn.read_exact(&mut buf).await?;
			Ok(String::from_utf8(buf).unwrap())
		} else {
			Ok(String::new())
		}
	} else {
		log::error!("recieved error {resp:?} from challd");
		Err(ChalldError::ResponseCode(resp))
	}
}
