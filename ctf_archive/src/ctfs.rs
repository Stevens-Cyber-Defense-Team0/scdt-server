use serde::{Serialize, Deserialize};
use axum::{Json, extract::State, http::StatusCode};
use utoipa::{OpenApi, ToSchema};
use diesel::QueryDsl;
use diesel::result::DatabaseErrorKind;
use diesel_async::RunQueryDsl;
use chrono::NaiveDate;
use crate::state::AppState;
use crate::date::Date;
use crate::models::NewCtf;
use crate::jwt::{JWTUser, JWTAdmin};

#[derive(OpenApi)]
#[openapi(
	paths(list, create),
	components(schemas(Ctf, CreateCtf))
)]
pub struct CtfsApi;

#[derive(Serialize, ToSchema)]
pub struct Ctf {
	id: i32,
	#[schema(example = "loremCTF 20xx")]
	name: String,

	start_date: Date,
	end_date: Date
}

#[utoipa::path(
	get,
	path = "list",
	operation_id = "ctf_list",
	responses(
		(status = OK, body = [Ctf], description = "list of all known CTFs")
	),
	security(
		("user_api_key" = [])
	),
	tag = "ctfs"
)]
pub async fn list(State(state): State<AppState>, _user: JWTUser) -> Json<Vec<Ctf>> {
	use crate::schema::ctfs::dsl;

	let mut conn = state.pool().get().await.unwrap();

	let ctfs: Vec<(i32, String, NaiveDate, NaiveDate)> = dsl::ctfs.select((dsl::id, dsl::name, dsl::start_date, dsl::end_date)).load(&mut conn).await.unwrap();

	Json(ctfs.into_iter().map(|(id, name, start_date, end_date)| Ctf {
		id,
		name,
		start_date: start_date.try_into().unwrap(),
		end_date: end_date.try_into().unwrap()
	}).collect())
}

#[derive(Deserialize, ToSchema)]
pub struct CreateCtf {
	#[schema(example = "loremCTF 20xx")]
	name: String,
	start_date: Date,
	end_date: Date
}

#[utoipa::path(
	post,
	path = "create",
	operation_id = "ctf_create",
	request_body = CreateCtf,
	responses(
		(status = CREATED, body = i32, description = "CTF was added successfully and its id is returned", content_type = "application/json"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action"),
		(status = CONFLICT, description = "CTF with the same name already exists")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "ctfs"
)]
pub async fn create(State(state): State<AppState>, admin: JWTAdmin, Json(ctf): Json<CreateCtf>) -> Result<(StatusCode, Json<i32>), StatusCode> {
	use crate::schema::ctfs::dsl;

	if !admin.can_create {
		return Err(StatusCode::FORBIDDEN);
	}

	let mut conn = state.pool().get().await.unwrap();

	let result = diesel::insert_into(dsl::ctfs).values(NewCtf {
		name: ctf.name,
		start_date: ctf.start_date.try_into().unwrap(),
		end_date: ctf.end_date.try_into().unwrap()
	}).returning(dsl::id).get_result(&mut conn).await;

	match result {
		Ok(id) => Ok((StatusCode::CREATED, Json(id))),
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			Err(StatusCode::CONFLICT)
		}
		Err(e) => {
			log::error!("{}", e);
			Err(StatusCode::INTERNAL_SERVER_ERROR)
		}
	}
}
