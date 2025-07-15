use serde::Deserialize;
use axum::{Json, extract::State, http::StatusCode};
use utoipa::{OpenApi, ToSchema};
use jwt_simple::prelude::Duration;
use jwt_simple::claims::Claims;
use base64::engine::{Engine, general_purpose::STANDARD as B64};
use p256::ecdsa::{VerifyingKey, Signature, signature::Verifier};
use hex_literal::hex;
use crate::jwt::{JWTChallenge, JWTAdmin};
use crate::state::AppState;

// ambitious potential future work: seal private key with TPM
macro_rules! admin_auth {
	($(($id:literal, $pubkey:literal, {$($p:ident: $v:literal),+})),+) => {
		fn key_exists(id: u8) -> bool {
			match id {
				$($id => true,)+
				_ => false
			}
		}

		fn key_perms(id: u8, challenge: &[u8], sig_bytes: &[u8]) -> Result<JWTAdmin, StatusCode> {
			match id {
				$($id => {
					let pubkey = VerifyingKey::from_sec1_bytes(&hex!($pubkey)).expect("use the macro correctly");
					if let Ok(sig) = Signature::from_slice(sig_bytes) {
						if pubkey.verify(&challenge, &sig).is_ok() {
							Ok(JWTAdmin {
								id: $id,
								$($p: $v),+
							})
						} else {
							Err(StatusCode::UNAUTHORIZED)
						}
					} else {
						Err(StatusCode::BAD_REQUEST)
					}
				},)+
				id => {
					log::warn!("somehow somebody was able to get a challenge for key id {id}");
					Err(StatusCode::INTERNAL_SERVER_ERROR)
				}
			}
		}
	}
}

admin_auth!(
	// debug key (python scripts)
	(0, "04d184cfb7cd2ad249bc41d20ae3484174d3a9f282bb3d7a1459fe10903098389220765700ff4c2fbbbbb63c85aa15079bfc80b695d7124081aaa1710bb4a4f932", {
		can_docker: true,
		can_create: true,
		can_invite: true,
		can_host_demo: true
	}),
	// discord bot key
	(1, "041f5b95b8a7c2e7d8db420f7a315049bffb9a5b772068786fc50d780c641da98b805bf097c40424c326e4c75946c5dc29a5655a84d6dbf1e97ead065f43037829", {
		can_docker: false,
		can_create: false,
		can_invite: true,
		can_host_demo: false
	})
);

#[derive(OpenApi)]
#[openapi(
	paths(initiate_challenge, finish_challenge),
	components(schemas(RequestAuthChallenge, ChallengeResponse))
)]
pub struct AuthApi;

#[derive(ToSchema, Deserialize)]
pub struct RequestAuthChallenge {
	#[schema(example = 0)]
	id: u8
}

#[utoipa::path(
	post,
	path = "initiate_challenge",
	operation_id = "auth_initiate_challenge",
	request_body = RequestAuthChallenge,
	responses(
		(status = OK, body = String, description = "a short-lived JWT containing the challenge in its claims"),
		(status = NOT_FOUND, description = "key with id does not exist")
	),
	tag = "auth"
)]
pub async fn initiate_challenge(State(state): State<AppState>, Json(login_request): Json<RequestAuthChallenge>) -> Result<String, StatusCode> {
	if !key_exists(login_request.id) {
		return Err(StatusCode::NOT_FOUND);
	}

	let mut challenge = vec![0; 32];
	getrandom::fill(&mut challenge).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
	let challenge = B64.encode(challenge);

	let claims = Claims::with_custom_claims(JWTChallenge {
		challenge,
		id: login_request.id
	}, Duration::from_secs(10));

	Ok(state.jwt_keys().authenticate(claims).unwrap())
}

// utoipa: `Byte` actually means base64 because that's what it was called in the OpenAPI 2 spec
#[derive(ToSchema, Deserialize)]
pub struct ChallengeResponse {
	#[schema(value_type = String, format = Byte)]
	r: String,
	#[schema(value_type = String, format = Byte)]
	s: String
}

#[utoipa::path(
	post,
	path = "finish_challenge",
	operation_id = "auth_finish_challenge",
	request_body = ChallengeResponse,
	responses(
		(status = OK, body = ChallengeResponse, description = "a session JWT which proves you are an administrator"),
		(status = BAD_REQUEST, description = "the signature is malformed"),
		(status = UNAUTHORIZED, description = "the signature is well-formed but invalid")
	),
	security(
		("challenge" = [])
	),
	tag = "auth"
)]
pub async fn finish_challenge(State(state): State<AppState>, jwt_challenge: JWTChallenge, Json(challenge_response): Json<ChallengeResponse>) -> Result<String, StatusCode> {
	let sig_bytes = {
			let (r, s) = match (B64.decode(challenge_response.r), B64.decode(challenge_response.s)) {
			(Ok(r), Ok(s)) => (r, s),
			_ => return Err(StatusCode::BAD_REQUEST)
		};

		let mut sig_bytes = r;
		sig_bytes.extend(s);
		sig_bytes
	};

	let challenge = B64.decode(jwt_challenge.challenge).unwrap();

	key_perms(jwt_challenge.id, &challenge, &sig_bytes).map(|admin_claims| {
		let claims = Claims::with_custom_claims(admin_claims, Duration::from_mins(30));
		state.jwt_keys().authenticate(claims).unwrap()
	})
}
