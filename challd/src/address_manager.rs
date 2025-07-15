use std::sync::atomic::{AtomicBool, Ordering};
use crate::MAX_INSTANCES;

pub struct AddressManager {
	used_addresses: [AtomicBool; MAX_INSTANCES]
}

impl AddressManager {
	pub fn new() -> Self {
		let mut used_addresses = Vec::with_capacity(MAX_INSTANCES);
		for _ in 0..MAX_INSTANCES {
			used_addresses.push(AtomicBool::new(false));
		}

		Self {
			used_addresses: used_addresses.try_into().unwrap()
		}
	}

	pub fn take_next_addr(&self) -> Option<u8> {
		for i in 0u8..(MAX_INSTANCES as u8) {
			let in_use = self.used_addresses[i as usize].fetch_or(true, Ordering::Relaxed);
			if !in_use {
				return Some(i + 1);
			}
		}

		None
	}

	pub fn relinquish_addr(&self, addr: u8) {
		assert!((1..=(MAX_INSTANCES as u8 + 1)).contains(&addr));
		self.used_addresses[(addr - 1) as usize].store(false, Ordering::Relaxed);
	}
}
