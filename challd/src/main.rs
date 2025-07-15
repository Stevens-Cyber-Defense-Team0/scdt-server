mod wg_config;

mod interface;
use interface::Interface;

mod crypto_stuff;
mod container;
mod address_manager;

mod dummy_interface;
use dummy_interface::DummyInterface;

mod ports;
use ports::PortMappings;

mod demo;
use demo::spawn_demo;

use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::signal::unix::SignalKind;
use tokio::process::Command;
use tokio_util::task::TaskTracker;
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use std::path::PathBuf;
use std::ffi::OsStr;

pub const MAX_INSTANCES: usize = 6;

const RESULT_OK: u8 = 0;
const RESULT_MALFORMED_REQUEST: u8 = 1;
const RESULT_INVALID_PORT: u8 = 2;
const RESULT_BUSY: u8 = 3;

const MODE_GUEST: u8 = 0;
const MODE_DEMO: u8 = 1;

enum Mode {
	GuestRequest(u32),
	Demo
}

macro_rules! handle_error {
	($s:ident, $f:ident, on_error=$e:ident) => {
		match $s.$f().await {
			Ok(v) => v,
			Err(_) => {
				let _ = $s.write_u8($e).await;
				return;
			}
		}
	};
	($s:ident, $f:expr, on_none=$e:ident) => {
		match $f {
			Some(v) => v,
			None => {
				let _ = $s.write_u8($e).await;
				return;
			}
		}
	}
}

#[tokio::main]
async fn main() {
	pretty_env_logger::init();

	let socket_path = std::env::var_os("SOCKET_PATH").as_deref().map(PathBuf::from).unwrap_or(PathBuf::from("socket"));
	let _dummy = DummyInterface::create();

	let addrs = Arc::new(address_manager::AddressManager::new());
	let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
	let int = Arc::new(Interface::new().unwrap());

	// TODO: fix this
	// I'm pretty sure there's actually a race condition here.
	// The `mio` crate calls the bind syscall before we change the permissons.
	// Unfortunately this is a massive pain to fix and it's not concerning enough to
	// justify spending my time fixing it.
	// The effect of this race condition being that a user without `challd` perms *might*
	// be able to send a valid message before we change the perms.
	// shouldn't actually be privillege escalation tho since we're only allowing a subset of
	// "safe" requests to actually reach the docker daemon.
	let listener = UnixListener::bind(&socket_path).unwrap();

	// TODO: use `std::os::unix::fs::fchown` instead
	#[cfg(feature = "challd_group")]
	assert!(Command::new("chown")
		.args([OsStr::new("root:challd"), socket_path.as_os_str()])
		.status().await.unwrap().success());

	// TODO: use `nix::sys::stat::fchmod` instead
	assert!(Command::new("chmod")
		.args([OsStr::new("720"), socket_path.as_os_str()])
		.status().await.unwrap().success());

	let tracker = Arc::new(TaskTracker::new());
	let token = CancellationToken::new();

	loop {
		tokio::select! {
			Ok((stream, _addr)) = listener.accept() => tokio::spawn({
				let addrs = addrs.clone();
				let int = int.clone();
				let token = token.clone();
				let tracker = tracker.clone();

				async move {
					let mut stream = BufReader::new(stream);

					let mode = {
						let mode = handle_error!(stream, read_u8, on_error=RESULT_MALFORMED_REQUEST);
						handle_error!(stream, match mode {
							MODE_GUEST => {
								let guest_code = handle_error!(stream, read_u32_le, on_error=RESULT_MALFORMED_REQUEST);
								Some(Mode::GuestRequest(guest_code))
							},
							MODE_DEMO => Some(Mode::Demo),
							_ => None
						}, on_none=RESULT_MALFORMED_REQUEST)
					};

					let len = handle_error!(stream, read_u8, on_error=RESULT_MALFORMED_REQUEST);

					let image_name = {
						let mut name = vec![0; len as usize];
						if stream.read_exact(&mut name).await.is_err() {
							let _ = stream.write_u8(RESULT_MALFORMED_REQUEST).await;
							return;
						}

						match String::from_utf8(name) {
							Ok(name) => name,
							Err(_) => {
								let _ = stream.write_u8(RESULT_MALFORMED_REQUEST).await;
								return;
							}
						}
					};

					let ports = handle_error!(stream, PortMappings::read(&mut stream).await, on_none=RESULT_INVALID_PORT);
					let maybe_addr = match mode {
						Mode::GuestRequest(guest_code) => {
							let addr = handle_error!(stream, addrs.take_next_addr(), on_none=RESULT_BUSY);
							log::info!("recieved request by {guest_code} to start {image_name:?} on addr {addr:?} with ports {ports:#?}");
							Some(addr)
						},
						Mode::Demo => {
							log::info!("recieved admin demo request to start {image_name:?} with ports {ports:#?}");
							None
						}
					};

					tracker.spawn(async move {
						let container = container::Container::create(image_name, maybe_addr, ports).await.unwrap();

						match mode {
							Mode::GuestRequest(guest_code) => {
								log::info!("user {guest_code} started container {:?}", container.id());
							},
							Mode::Demo => {
								log::info!("admin started container {:?} for a demo", container.id());
							}
						}

						let shutdown_signal = container.shutdown_signal();

						let cleanup = async move {
							log::info!("stopping container...");
							container.shutdown().await;
							log::info!("stopped container");

							if let Some(addr) = maybe_addr {
								log::debug!("relinquishing address...");
								addrs.relinquish_addr(addr);
								log::debug!("relinquished address");
							}
						};

						let (handle, maybe_cfg) = match maybe_addr {
							Some(addr) => {
								let (handle, cfg) = int.add_temp_peer(
									cleanup,
									core::time::Duration::from_secs(4 * 60 * 60),
									addr,
									token,
									shutdown_signal
								).unwrap();

								(handle, Some(cfg))
							},
							None => (spawn_demo(
								cleanup,
								core::time::Duration::from_secs(6 * 60 * 60),
								token,
								shutdown_signal
							), None)
						};

						// as the daemon, we don't really care if this is 100% reliable
						// and we sure as hell don't need to wait for clients to read our responses while shutting down
						tokio::spawn(async move {
							let _ = stream.write_u8(RESULT_OK).await;

							if let Some(cfg) = maybe_cfg {
								let cfg = cfg.serialize();
								let len = cfg.len().try_into().unwrap();
								stream.write_u16_le(len).await.unwrap();
								stream.write_all(cfg.as_bytes()).await.unwrap();
							}

							stream.shutdown().await.unwrap();
						});

						handle.await.unwrap().unwrap();
					});
				}
			}),
			Ok(()) = tokio::signal::ctrl_c() => {
				break
			},
			Some(()) = sigterm.recv() => {
				break
			}
		};
	}

	token.cancel();
	tracker.close();
	tracker.wait().await;
	drop(listener);
	std::fs::remove_file(socket_path).unwrap();
}
