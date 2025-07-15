use axum::{Json, extract::State, http::StatusCode};
use utoipa::OpenApi;
use jwt_simple::prelude::Duration;
use jwt_simple::claims::Claims;
use serde::{Serialize, Deserialize};
use crate::jwt::{JWTAdmin, JWTUser};
use crate::state::AppState;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct GuestCode(pub u32);

#[derive(OpenApi)]
#[openapi(
	paths(generate_code, wipe_codes, redeem_code)
)]
pub struct GuestApi;

#[utoipa::path(
	post,
	path = "generate_code",
	operation_id = "guest_generate_code",
	request_body(content = String, description = "optional data that will be logged for auditing purposes", example = "generated through Swagger UI"),
	responses(
		(status = OK, body = u32, description = "returns a guest invite code which can be redeemed for a user session, the code expires 30 minutes from creation time", content_type = "application/json"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "guest"
)]
pub async fn generate_code(State(state): State<AppState>, admin: JWTAdmin, audit_data: String) -> Result<Json<GuestCode>, StatusCode> {
	const THIRTY_MINUTES_AS_SECS: u64 = 30 * 60;

	if !admin.can_invite {
		return Err(StatusCode::FORBIDDEN);
	}

	let code = GuestCode(getrandom::u32().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?);

	match audit_data.is_empty() {
		true => log::info!("admin {} generated {code:?} (with no associated audit data)", admin.id),
		false => log::info!("admin {} generated {code:?} ({audit_data:?})", admin.id)
	}

	let codes = state.codes();

	// technically this is only probably correct
	// if this ever breaks I will be astonished
	codes.insert_temp(code, std::time::Duration::from_secs(THIRTY_MINUTES_AS_SECS));

	Ok(Json(code))
}

#[utoipa::path(
	delete,
	path = "wipe_codes",
	operation_id = "guest_wipe_codes",
	responses(
		(status = OK, description = "all codes have been revoked (although not necessarily the sessions they granted)")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "guest"
)]
pub async fn wipe_codes(State(state): State<AppState>, _admin: JWTAdmin) -> StatusCode {
	let codes = state.codes();
	codes.wipe();
	StatusCode::OK
}

#[utoipa::path(
	post,
	path = "redeem_code",
	operation_id = "guest_redeem_code",
	request_body(content = u32, description = "a guest invite code", content_type = "application/json"),
	responses(
		(status = OK, description = "returns a valid user jwt", content_type = "text/plain"),
		(status = UNAUTHORIZED, description = "guest invite code invalid")
	),
	tag = "guest"
)]
pub async fn redeem_code(State(state): State<AppState>, Json(code): Json<GuestCode>) -> Result<String, StatusCode> {
	let codes = state.codes();

	let code_valid = codes.remove(&code);
	if code_valid {
		let claims = Claims::with_custom_claims(JWTUser { code }, Duration::from_hours(6));

		Ok(state.jwt_keys().authenticate(claims).unwrap())
	} else {
		Err(StatusCode::UNAUTHORIZED)
	}
}
