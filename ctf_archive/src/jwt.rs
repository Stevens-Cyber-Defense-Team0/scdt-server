use jwt_simple::prelude::*;
use axum::extract::FromRequestParts;
use axum::http::{request::Parts, StatusCode, header::HeaderValue};
use crate::state::AppState;
use crate::guest::GuestCode;

mod private {
	use serde::{Serialize, Deserialize};
	use jwt_simple::prelude::HS256Key;
	use crate::jwt::JWTKeys;

	pub trait KeyedJWT: Serialize + for<'a> Deserialize<'a> {
		fn key(jwt_keys: &JWTKeys) -> &HS256Key;
	}
}

pub struct JWTKeys {
	admin_key: HS256Key,
	challenge_key: HS256Key,
	user_key: HS256Key
}

impl JWTKeys {
	pub fn generate() -> Self {
		Self {
			admin_key: HS256Key::generate(),
			challenge_key: HS256Key::generate(),
			user_key: HS256Key::generate()
		}
	}

	pub fn authenticate<C>(&self, claims: JWTClaims<C>) -> Result<String, jwt_simple::Error>
	where
		C: private::KeyedJWT
	{
		C::key(self).authenticate(claims)
	}
}

#[derive(Serialize, Deserialize)]
pub struct JWTAdmin {
	pub id: u8,
	/// can configure docker stuff
	pub can_docker: bool,
	/// can create and modify ctfs, categories, and challenges (but can't necessarily configure docker)
	pub can_create: bool,
	/// can create new guest codes
	pub can_invite: bool,
	/// can host demos (DANGEROUS, this allows you to punch holes in the firewall!)
	pub can_host_demo: bool
}

#[derive(Serialize, Deserialize)]
pub struct JWTChallenge {
	pub challenge: String,
	pub id: u8
}

#[derive(Serialize, Deserialize)]
pub struct JWTUser {
	pub code: GuestCode
}

macro_rules! impl_jwt {
	($t:ty, $k:ident, $h:literal) => {
		impl private::KeyedJWT for $t {
			fn key(jwt_keys: &JWTKeys) -> &HS256Key {
				&jwt_keys.$k
			}
		}

		impl FromRequestParts<AppState> for $t {
			type Rejection = StatusCode;

			async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
				let key = <$t as private::KeyedJWT>::key(state.jwt_keys());

				if let Some(Ok(jwt_header)) = parts.headers.get($h).map(HeaderValue::to_str) {
					match key.verify_token::<Self>(jwt_header, None).ok().map(|claims| claims.custom) {
						Some(claims) => Ok(claims),
						None => Err(StatusCode::UNAUTHORIZED)
					}
				} else {
					Err(StatusCode::UNAUTHORIZED)
				}
			}
		}
	}
}

impl_jwt!(JWTChallenge, challenge_key, "challenge");
impl_jwt!(JWTAdmin, admin_key, "admin_jwt");
impl_jwt!(JWTUser, user_key, "user_jwt");
