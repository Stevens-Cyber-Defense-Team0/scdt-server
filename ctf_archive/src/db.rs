use diesel::{Connection, pg::PgConnection};
use diesel_migrations::{MigrationHarness, EmbeddedMigrations, embed_migrations};
use diesel_async::AsyncPgConnection;

pub type Pool = diesel_async::pooled_connection::bb8::Pool<AsyncPgConnection>;

#[cfg(not(feature = "production"))]
pub const DB_URL: &str = "postgres://ctf_archive:hunter2@127.0.0.1/ctf_archive";

#[cfg(feature = "production")]
pub const DB_URL: &str = "postgres://ctf_archive@%2Frun%2Fpostgresql/ctf_archive";

pub fn run_migrations() {
	let mut conn = PgConnection::establish(DB_URL).unwrap();
	const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
	conn.run_pending_migrations(MIGRATIONS).unwrap();
}
