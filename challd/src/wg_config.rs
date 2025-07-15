pub struct WgConfig {
	pub interface: WgInterface,
	pub peer: WgPeer
}

impl WgConfig {
	pub fn serialize(self) -> String {
		format!("\
[Interface]
Address = {}
PrivateKey = {}

[Peer]
PublicKey = {}
Endpoint = {}
AllowedIPs = {}", self.interface.address, self.interface.private_key, self.peer.public_key, self.peer.endpoint, self.peer.allowed_ips)
	}
}

// TODO: make this type-safe
pub struct WgInterface {
	pub address: String,
	pub private_key: String,
}

// TODO: make this type-safe
pub struct WgPeer {
	pub public_key: String,
	pub endpoint: String,
	pub allowed_ips: String
}
