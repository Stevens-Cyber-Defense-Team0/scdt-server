use tokio::io::AsyncReadExt;
use bollard::models::PortBinding;
use std::collections::HashMap;

#[derive(Debug)]
pub enum PortType {
	Tcp,
	Udp
}

impl PortType {
	fn as_string(&self, port: u16) -> String {
		match self {
			Self::Tcp => format!("{port}/tcp"),
			Self::Udp => format!("{port}/udp")
		}
	}
}

#[derive(Debug)]
pub struct PortMapping {
	pub from_port: u16,
	pub to_port: u16,
	pub port_type: PortType
}

#[derive(Debug)]
pub struct PortMappings(Vec<PortMapping>);

impl PortMappings {
	pub async fn read<S: AsyncReadExt + Unpin>(stream: &mut S) -> Option<Self> {
		let num_mappings = stream.read_u8().await.ok()?;

		if num_mappings >= 10 {
			return None;
		}

		let mut mappings = Vec::with_capacity(num_mappings as usize);

		for _ in 0..num_mappings {
			let from_port = stream.read_u16_le().await.ok()?;
			let to_port = stream.read_u16_le().await.ok()?;
			let port_type = match stream.read_u8().await.ok()? {
				0 => PortType::Tcp,
				1 => PortType::Udp,
				_ => return None
			};

			mappings.push(PortMapping {
				from_port,
				to_port,
				port_type
			});
		}

		Some(Self(mappings))
	}
}

impl PortMappings {
	pub fn into_hashmap(self, addr: Option<u8>) -> HashMap<String, Option<Vec<PortBinding>>> {
		let mut hashmap = HashMap::with_capacity(self.0.len());

		for mapping in self.0.into_iter() {
			let binding = PortBinding {
				host_ip: Some(match addr {
					Some(addr) => format!("10.4.2.{addr}"),
					None => String::from("0.0.0.0") // this is kinda naive and scuffed but it works well enough
				}),
				host_port: Some(mapping.port_type.as_string(mapping.to_port))
			};

			hashmap.insert(mapping.port_type.as_string(mapping.from_port), Some(vec![binding]));
		}

		hashmap
	}
}
