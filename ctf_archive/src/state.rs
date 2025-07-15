use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};
use std::sync::Arc;
use crate::guest::GuestCode;

pub struct InternalState {
	pool: crate::db::Pool,
	jwt_keys: crate::jwt::JWTKeys,
	codes: crate::codes::InMemorySet<GuestCode>
}

pub type AppState = Arc<InternalState>;

impl InternalState {
	pub async fn new() -> AppState {
		let pool = crate::db::Pool::builder().build(
			AsyncDieselConnectionManager::<AsyncPgConnection>::new(crate::db::DB_URL)
		).await.unwrap();

		let jwt_keys = crate::jwt::JWTKeys::generate();
		let codes = crate::codes::InMemorySet::default();

		Arc::new(InternalState {
			pool,
			jwt_keys,
			codes
		})
	}

	pub fn pool(&self) -> &crate::db::Pool {
		&self.pool
	}

	pub fn jwt_keys(&self) -> &crate::jwt::JWTKeys {
		&self.jwt_keys
	}

	pub fn codes(&self) -> &crate::codes::InMemorySet<GuestCode> {
		&self.codes
	}
}
