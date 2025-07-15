use axum::response::Html;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use core::net::{SocketAddr, IpAddr, Ipv4Addr};

// db stuff
mod db;
mod schema;
mod models;

// authentication stuff
mod security_addon;
mod jwt;
mod auth;
mod guest;

// common API types
mod date;
mod binary;

// api routes
mod ctfs;
mod categories;
mod challenges;

// whatever else I feel like
mod state;
mod codes;
mod challd;

use security_addon::SecurityAddon;

#[derive(OpenApi)]
#[openapi(
	info(
		title = "SCDT CTF Archive API",
		description = "So you wanna play an old CTF challenge?  This is the API you wanna be using."
	),
	servers(
		(url = "http://127.0.0.1:8080", description = "development")
	),
	components(schemas(date::Date, binary::Binary)),
	modifiers(&SecurityAddon),
	nest(
		(path = "/api/ctfs/", api = ctfs::CtfsApi),
		(path = "/api/categories/", api = categories::CategoriesApi),
		(path = "/api/challenges/", api = challenges::ChallengesApi),
		(path = "/api/auth/", api = auth::AuthApi),
		(path = "/api/guest/", api = guest::GuestApi)
	),
	tags(
		(name = "ctfs", description = "endpoint for CTF events"),
		(name = "categories", description = "endpoint for CTF categories"),
		(name = "challenges", description = "endpoint for CTF challenges"),
		(name = "auth", description = "endpoint for authentication")
	)
)]
#[cfg_attr(feature = "production", openapi(
	servers(
		(url = "https://scdt.club", description = "production")
	)
))]
struct ApiDoc;

const PORT: u16 = 8080;
const BIND_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), PORT);

#[tokio::main]
async fn main() {
	pretty_env_logger::init();

	db::run_migrations();

	let state = state::InternalState::new().await;

	// we don't rely on cookies for security, so I believe this is acceptable
	let cors = tower_http::cors::CorsLayer::new()
		.allow_origin(tower_http::cors::Any)
		.allow_methods(tower_http::cors::Any)
		.allow_headers(tower_http::cors::Any);

	let app = axum::Router::new()
		.route("/", axum::routing::get(index))
		.route("/api/ctfs/list", axum::routing::get(ctfs::list))
		.route("/api/ctfs/create", axum::routing::post(ctfs::create))
		.route("/api/categories/get", axum::routing::get(categories::get))
		.route("/api/categories/create", axum::routing::post(categories::create))
		.route("/api/challenges/list", axum::routing::get(challenges::list))
		.route("/api/challenges/get_metadata", axum::routing::get(challenges::get_metadata))
		.route("/api/challenges/create", axum::routing::post(challenges::create))
		.route("/api/challenges/get_archive", axum::routing::get(challenges::get_archive))
		.route("/api/challenges/start", axum::routing::post(challenges::start))
		.route("/api/challenges/configure_docker", axum::routing::put(challenges::configure_docker))
		.route("/api/challenges/get_flag", axum::routing::get(challenges::get_flag))
		.route("/api/challenges/set_connection_instructions", axum::routing::post(challenges::set_connection_instructions))
		.route("/api/challenges/host_demo", axum::routing::post(challenges::host_demo))
		.route("/api/challenges/set_archive", axum::routing::put(challenges::set_archive))
		.route("/api/auth/initiate_challenge", axum::routing::post(auth::initiate_challenge))
		.route("/api/auth/finish_challenge", axum::routing::post(auth::finish_challenge))
		.route("/api/guest/generate_code", axum::routing::post(guest::generate_code))
		.route("/api/guest/wipe_codes", axum::routing::delete(guest::wipe_codes))
		.route("/api/guest/redeem_code", axum::routing::post(guest::redeem_code))
		.with_state(state)
		.layer(cors)
		.merge(
			SwaggerUi::new("/swagger").url("/api/openapi.json", ApiDoc::openapi())
		);

	let listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await.unwrap();
	axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<&'static str> {
	Html(include_str!("../static/index.html"))
}
