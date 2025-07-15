use utoipa::Modify;
use utoipa::openapi::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};

pub struct SecurityAddon;

impl Modify for SecurityAddon {
	fn modify(&self, openapi: &mut OpenApi) {
		let components = openapi.components.as_mut().unwrap();

		components.add_security_scheme(
			"user_api_key",
			SecurityScheme::ApiKey(ApiKey::Header(
				ApiKeyValue::with_description("user_jwt", "a JWT obtained by logging in normally")
			))
		);

		components.add_security_scheme(
			"admin_api_key",
			SecurityScheme::ApiKey(ApiKey::Header(
				ApiKeyValue::with_description("admin_jwt", "a JWT obtained by jumping through a flaming hoop or something idk")
			))
		);

		components.add_security_scheme(
			"challenge",
			SecurityScheme::ApiKey(ApiKey::Header(
				ApiKeyValue::with_description("challenge", "a JWT obtained by initiating a challenge")
			))
		);
	}
}
