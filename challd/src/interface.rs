use defguard_wireguard_rs as wg;
use wg::{WireguardInterfaceApi, InterfaceConfiguration};

use core::time::Duration;
use std::sync::Arc;
use base64::engine::{Engine, general_purpose::STANDARD as B64};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use thiserror::Error;
use crate::crypto_stuff;
use crate::wg_config as wgc;
use crate::MAX_INSTANCES;

const DEVICE_NAME: &str = "wg42";

pub type AddPeerResult = Result<(JoinHandle<Result<(), RemovePeerError>>, wgc::WgConfig), wg::error::WireguardInterfaceError>;

#[derive(Error, Debug)]
pub enum RemovePeerError {
	#[error(transparent)]
	Interface(wg::error::WireguardInterfaceError)
}

pub struct Interface {
	wgapi: wg::WGApi<wg::Kernel>,
	pubkey: String
}

const INT_ADDR_MASK: wg::net::IpAddrMask = wg::net::IpAddrMask {
	ip: core::net::IpAddr::V4(
		core::net::Ipv4Addr::new(10, 4, 1, 0)
	),
	cidr: 24
};

impl Interface {
	pub fn new() -> Result<Self, wg::error::WireguardInterfaceError> {
		let wgapi = wg::WGApi::<wg::Kernel>::new(DEVICE_NAME.to_string())?;
		wgapi.create_interface()?;

		let host_keypair = crypto_stuff::gen_keypair();

		let mut prvkey = String::new();
		B64.encode_string(host_keypair.privkey, &mut prvkey);

		wgapi.configure_interface(&InterfaceConfiguration {
			name: DEVICE_NAME.to_string(),
			prvkey,
			addresses: vec![INT_ADDR_MASK],
			port: 51820,
			peers: vec![],
			mtu: None
		}).unwrap();

		let mut pubkey_b64 = String::new();
		B64.encode_string(host_keypair.pubkey, &mut pubkey_b64);

		Ok(Self {
			wgapi,
			pubkey: pubkey_b64
		})
	}

	pub fn add_temp_peer<C, D>(
		self: &Arc<Self>,
		cleanup: C,
		duration: Duration,
		addr: u8,
		token: CancellationToken,
		shutdown_signal: D
	) -> AddPeerResult
	where
		C: core::future::Future<Output = ()> + Send + 'static,
		D: core::future::Future<Output = ()> + Send + 'static
	{
		assert!((1..=(MAX_INSTANCES as u8 + 1)).contains(&addr));
		let peer_keypair = crypto_stuff::gen_keypair();
		let public_key = wg::key::Key::new(peer_keypair.pubkey.to_bytes());

		self.wgapi.configure_peer(&wg::host::Peer {
			public_key: public_key.clone(),
			preshared_key: None,
			protocol_version: None,
			endpoint: None,
			last_handshake: None,
			tx_bytes: 0,
			rx_bytes: 0,
			persistent_keepalive_interval: None,
			allowed_ips: vec![wg::net::IpAddrMask {
				ip: core::net::IpAddr::V4(format!("10.4.1.{addr}").parse().unwrap()),
				cidr: 32
			}]
		})?;

		let mut private_key = String::new();
		B64.encode_string(peer_keypair.privkey, &mut private_key);

		let s = self.clone();
		let f = tokio::spawn(async move {
			tokio::select! {
				_ = tokio::time::sleep(duration) => (),
				_ = token.cancelled() => (),
				_ = shutdown_signal => ()
			}

			if let Err(e) = s.remove_peer(&public_key) {
				return Err(RemovePeerError::Interface(e));
			}

			cleanup.await;

			Ok(())
		});

		Ok((f, wgc::WgConfig {
			interface: wgc::WgInterface {
				address: format!("10.4.1.{addr}/32"),
				private_key
			},
			peer: wgc::WgPeer {
				public_key: self.pubkey.clone(),
				#[cfg(not(feature = "domain_name"))]
				endpoint: String::from("127.0.0.1:51820"),
				#[cfg(feature = "domain_name")]
				endpoint: String::from("scdt.club:51820"),
				allowed_ips: String::from("10.4.2.0/32")
			}
		}))
	}

	fn remove_peer(&self, peer: &wg::key::Key) -> Result<(), wg::error::WireguardInterfaceError> {
		self.wgapi.remove_peer(peer)
	}
}

impl Drop for Interface {
	fn drop(&mut self) {
		self.wgapi.remove_interface().unwrap();
	}
}
