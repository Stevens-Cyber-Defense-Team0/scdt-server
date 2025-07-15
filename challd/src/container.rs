use bollard::{Docker, errors::Error, models::HostConfig};
use bollard::models::ContainerCreateBody;
use bollard::query_parameters::{StartContainerOptions, WaitContainerOptions, CreateContainerOptions, InspectContainerOptions, StopContainerOptions};
use bollard::API_DEFAULT_VERSION;
use tokio::process::Command;
use futures_util::StreamExt;
use crate::ports::PortMappings;
use crate::MAX_INSTANCES;

const DEFAULT_SOCKET: &str = "unix:///var/run/docker.sock";

struct VpnInfo {
	addr: u8,
	bridge_addr: String
}

pub struct Container {
	id: Option<String>,
	vpn_info: Option<VpnInfo>
}

const MAX_MEM: i64 = 100 * 1024 * 1024;

impl Container {
	pub async fn create(image_name: String, maybe_addr: Option<u8>, ports: PortMappings) -> Result<Self, Error> {
		if let Some(addr) = maybe_addr {
			assert!((1..=(MAX_INSTANCES as u8 + 1)).contains(&addr));
		}

		let d = Docker::connect_with_unix_defaults()?;

		if let Some(addr) = maybe_addr {
			assert!(Command::new("ip")
				.args(["addr", "add", &format!("10.4.2.{addr}/32"), "dev", "eth42"])
				.status().await.unwrap().success());
		}

		let id = d.create_container(None::<CreateContainerOptions>, ContainerCreateBody {
			image: Some(image_name),
			stop_timeout: Some(10),
			host_config: Some(HostConfig {
				auto_remove: Some(true),
				memory: Some(MAX_MEM),
				memory_swap: Some(MAX_MEM),
				nano_cpus: Some(2 * 10_i64.pow(8)), // 0.2 CPUs
				pids_limit: Some(150), // limit fork bomb trolling
				port_bindings: Some(ports.into_hashmap(maybe_addr)),
				#[cfg(feature = "gvisor")]
				runtime: Some(String::from("runsc")),
				..Default::default()
			}),
			..Default::default()
		}).await.unwrap().id;

		d.start_container(&id, None::<StartContainerOptions>).await.unwrap();

		let vpn_info = match maybe_addr {
			Some(addr) => {
				let networks = d.inspect_container(&id, None::<InspectContainerOptions>).await.unwrap().network_settings.unwrap().networks.unwrap();
				let bridge = networks.get("bridge").unwrap();
				let bridge_addr = bridge.ip_address.as_ref().unwrap().clone();

				assert!(Command::new("iptables")
					.args(["-t", "nat", "-A", "PREROUTING", "-i", "wg42", "-s", &format!("10.4.1.{addr}/32"), "-d", "10.4.2.0", "-j", "DNAT", "--to-destination", &bridge_addr])
					.status().await.unwrap().success());

				Some(VpnInfo {
					addr,
					bridge_addr
				})
			},
			None => None
		};

		Ok(Self {
			id: Some(id),
			vpn_info
		})
	}

	pub async fn shutdown(mut self) {
		// set timeout to 15 minutes, we really really really need to be absolutely sure we stop containers so we don't leak memory
		let d = Docker::connect_with_unix(DEFAULT_SOCKET, 15 * 60, API_DEFAULT_VERSION).unwrap();

		let id = self.id.take().expect("container must be running to shutdown");
		match d.stop_container(&id, None::<StopContainerOptions>).await {
			Ok(()) => (),
			Err(Error::DockerResponseServerError { status_code: 404 , .. }) => {
				match self.vpn_info.as_ref().map(|v| v.addr) {
					Some(vpn_addr) => log::warn!("container {id:?} on address {vpn_addr:?} was stopped early"),
					None => log::warn!("demo container {id:?} was stopped early")
				}
			},
			Err(e) => {
				log::error!("failed to stop container {:?}", e);
			}
		}

		if let Some(vpn) = &self.vpn_info {
			assert!(Command::new("iptables")
				.args(["-t", "nat", "-D", "PREROUTING", "-i", "wg42", "-s", &format!("10.4.1.{}/32", vpn.addr), "-d", "10.4.2.0", "-j", "DNAT", "--to-destination", &vpn.bridge_addr])
				.status().await.unwrap().success());

			assert!(Command::new("ip")
				.args(["addr", "del", &format!("10.4.2.{}/32", vpn.addr), "dev", "eth42"])
				.status().await.unwrap().success());
		}
	}

	pub fn id(&self) -> &str {
		match self.id {
			Some(ref id) => id,
			None => unreachable!()
		}
	}

	pub fn shutdown_signal(&self) -> impl core::future::Future<Output = ()> + Send + 'static {
		let id = match self.id {
			Some(ref id) => id.clone(),
			None => unreachable!()
		};

		Self::shutdown_signal_impl(id)
	}

	async fn shutdown_signal_impl(id: String) {
		let d = match Docker::connect_with_unix_defaults() {
			Ok(d) => d,
			Err(_) => return
		};

		let mut shutdown_stream = d.wait_container(&id, Some(WaitContainerOptions {
			condition: String::from("not-running")
		}));

		while shutdown_stream.next().await.is_some() {
			// eat all events, wait for stream to terminate
		}
	}
}

impl Drop for Container {
	fn drop(&mut self) {
		assert!(self.id.is_none(), "container should have been shutdown properly!");
	}
}
