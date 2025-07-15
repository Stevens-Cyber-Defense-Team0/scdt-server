use std::sync::{Mutex, Arc};
use std::collections::HashSet;
use std::borrow::Borrow;
use std::hash::Hash;
use std::time::Duration;

pub struct InMemorySet<T> {
	set: Arc<Mutex<HashSet<T>>>
}

impl<T> Default for InMemorySet<T> {
	fn default() -> Self {
		Self {
			set: Arc::new(Mutex::new(HashSet::default()))
		}
	}
}

impl<T: Eq + Hash> InMemorySet<T> {
	pub fn insert_temp(&self, val: T, duration: Duration)
	where
		T: Clone + Send + 'static
	{
		let val2 = val.clone();

		{
			let mut set = self.set.lock().unwrap();
			set.insert(val);
		}

		tokio::spawn({
			let set = self.set.clone();

			async move {
				tokio::time::sleep(duration).await;

				match set.lock() {
					Ok(mut set) => { set.remove(&val2); },
					Err(e) => {
						log::error!("failed to acquire lock on in-memory set ({e:?}), unable to remove temporary insertion");
					}
				};
			}
		});
	}

	/// removes `val` from the set and returns `true` if and only if the value was previously in the set
	pub fn remove<Q>(&self, val: &Q) -> bool
	where
		T: Borrow<Q>,
		Q: Hash + Eq + ?Sized
	{
		let mut set = self.set.lock().unwrap();
		set.remove(val)
	}

	pub fn wipe(&self) {
		let mut set = self.set.lock().unwrap();
		set.clear();
	}
}
