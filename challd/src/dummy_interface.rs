pub struct DummyInterface;

impl DummyInterface {
	pub fn create() -> Self {
		assert!(std::process::Command::new("ip")
			.args(["link", "add", "eth42", "type", "dummy"])
			.status().unwrap().success());

		assert!(std::process::Command::new("ip")
			.args(["link", "set", "dev", "eth42", "up"])
			.status().unwrap().success());

		Self
	}
}

impl Drop for DummyInterface {
	fn drop(&mut self) {
		std::process::Command::new("ip")
			.args(["link", "delete", "eth42", "type", "dummy"])
			.status().unwrap();
	}
}
