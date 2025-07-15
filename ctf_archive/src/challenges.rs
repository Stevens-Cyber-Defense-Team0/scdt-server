use serde::{Serialize, Deserialize};
use axum::{Json, extract::{State, Query, Multipart, OptionalFromRequest, Request}, http::{StatusCode, header}, body::Body};
use utoipa::{OpenApi, ToSchema};
use diesel::{QueryDsl, ExpressionMethods};
use diesel::result::DatabaseErrorKind;
use diesel_async::RunQueryDsl;
use futures_util::TryStreamExt;
use std::collections::HashMap;
use crate::state::AppState;
use crate::models::{NewChallenge, PortType};
use crate::binary::Binary;
use crate::jwt::{JWTUser, JWTAdmin};
use crate::challd;

#[derive(OpenApi)]
#[openapi(
	paths(list, get_metadata, create, get_archive, start, configure_docker, get_flag, set_connection_instructions, host_demo, set_archive),
	components(schemas(Challenge, CreateChallengeFile, ExposePorts, ExposePortType))
)]
pub struct ChallengesApi;

#[derive(Serialize, ToSchema)]
pub struct Challenge {
	id: i32,
	#[schema(example = "cereal killer 1")]
	name: String,
	category_id: i32,
	#[schema(example = "blah blah I love cereal blah")]
	description: String,
	#[schema(example = "easy")]
	difficulty: String,
	#[schema(example = "nc chall.scdt.club 1234")]
	connection_instructions: Option<String>,
	has_archive: bool
}

#[derive(Deserialize)]
pub struct ListChallengeQuery {
	category_id: i32
}

#[utoipa::path(
	get,
	path = "list",
	operation_id = "challenge_list",
	params(
		("category_id" = i32, Query, description = "the category id to list challenges of")
	),
	responses(
		(status = OK, body = [i32], description = "returns a list of challenge ids in the requested category, if the category doesn't exist `[]` will be returned", content_type = "application/json")
	),
	security(
		("user_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn list(State(state): State<AppState>, query: Query<ListChallengeQuery>, _user: JWTUser) -> Result<Json<Vec<i32>>, StatusCode> {
	use crate::schema::challenges::dsl;

	let mut conn = state.pool().get().await.unwrap();

	let result = dsl::challenges.select(dsl::id).filter(dsl::category_id.eq(query.category_id)).get_results(&mut conn).await;

	let challenges: Vec<i32> = match result {
		Ok(challenges) => challenges,
		Err(e) => {
			log::error!("{}", e);
			return Err(StatusCode::INTERNAL_SERVER_ERROR)
		}
	};

	Ok(Json(challenges))
}

#[derive(Deserialize)]
pub struct ChallengeByIdQuery {
	challenge_id: i32
}

#[utoipa::path(
	get,
	path = "get_metadata",
	operation_id = "challenge_get_metadata",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to get metadata about")
	),
	responses(
		(status = OK, body = Challenge, description = "returns metadata about the requested challenge"),
		(status = NOT_FOUND, description = "challenge with requested id doesn't exist")
	),
	security(
		("user_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn get_metadata(State(state): State<AppState>, query: Query<ChallengeByIdQuery>, _user: JWTUser) -> Result<Json<Challenge>, StatusCode> {
	use crate::schema::challenges::dsl;

	let mut conn = state.pool().get().await.unwrap();

	let result = dsl::challenges.find(query.challenge_id).select((dsl::id, dsl::name, dsl::description, dsl::difficulty, dsl::connection_instructions, dsl::archive.is_not_null())).get_result(&mut conn).await;

	let (id, name, description, difficulty, connection_instructions, has_archive): (i32, String, String, String, Option<String>, bool) = match result {
		Ok(challenge) => challenge,
		Err(diesel::result::Error::NotFound) => {
			return Err(StatusCode::NOT_FOUND);
		},
		Err(e) => {
			log::error!("{}", e);
			return Err(StatusCode::INTERNAL_SERVER_ERROR);
		}
	};

	Ok(Json(Challenge {
		id,
		name,
		category_id: query.challenge_id,
		description,
		difficulty,
		connection_instructions,
		has_archive
	}))
}

#[derive(ToSchema)]
pub struct CreateChallengeFile {
	#[schema(value_type = String, format = Binary, required = false, content_media_type = "application/zip")]
	#[allow(dead_code)] // this field is strictly for OpenAPI docs
	zip_file: Option<Vec<u8>>
}

#[derive(Deserialize)]
pub struct CreateChallenge {
	name: String,
	flag: Option<String>,
	category_id: i32,
	description: String,
	difficulty: String,
	connection_instructions: Option<String>
}

#[utoipa::path(
	post,
	path = "create",
	operation_id = "challenge_create",
	params(
		("name" = String, Query, description = "the name of the challenge"),
		("flag" = Option<String>, Query, description = "the flag of the challenge"),
		("category_id" = i32, Query, description = "the id of the category to create this challenge in"),
		("description" = String, Query, description = "the description of the challenge (preferably the official description, if not possible then write your own description without spoilers and note that it's not the original)"),
		("difficulty" = String, Query, description = "the difficulty of the challenge (preferably whatever the CTF uses to describe difficulty, otherwise just make something up and note it in the description)")
	),
	request_body(content = CreateChallengeFile, description = "a zip file containing all supplied challenge files (not required)", content_type = "multipart/form-data"),
	responses(
		(status = CREATED, body = i32, description = "challenge was added successfully and its id is returned", content_type = "application/json"),
		(status = BAD_REQUEST, description = "archive not uploaded correctly"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action"),
		(status = CONFLICT, description = "challenge with the same name already exists in this category"),
		(status = NOT_FOUND, description = "category with requested id does not exist")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn create(State(state): State<AppState>, Query(query): Query<CreateChallenge>, admin: JWTAdmin, mut multi: Multipart) -> Result<(StatusCode, Json<i32>), StatusCode> {
	use crate::schema::challenges::dsl;

	if !admin.can_create {
		return Err(StatusCode::FORBIDDEN);
	}

	let archive = match multi.next_field().await {
		Ok(Some(field)) if
			field.name() == Some("zip_file") &&
			field.content_type() == Some("application/zip") =>
		{
			if let Ok(bytes) = field.bytes().await {
				Some(bytes.into())
			} else{
				return Err(StatusCode::BAD_REQUEST);
			}
		},
		Ok(Some(_)) => return Err(StatusCode::BAD_REQUEST),
		Ok(None) => None,
		Err(_) => return Err(StatusCode::BAD_REQUEST)
	};

	let mut conn = state.pool().get().await.unwrap();

	let chall = NewChallenge {
		name: query.name,
		flag: query.flag,
		category_id: query.category_id,
		description: query.description,
		difficulty: query.difficulty,
		archive,
		port_numbers: vec![],
		port_types: vec![],
		connection_instructions: query.connection_instructions
	};

	let result = diesel::insert_into(dsl::challenges).values(chall)
		.returning(dsl::id).get_result(&mut conn).await;

	match result {
		Ok(id) => Ok((StatusCode::CREATED, Json(id))),
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			Err(StatusCode::CONFLICT)
		},
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _)) => {
			Err(StatusCode::NOT_FOUND)
		},
		Err(e) => {
			log::error!("{}", e);
			Err(StatusCode::INTERNAL_SERVER_ERROR)
		}
	}
}

#[utoipa::path(
	get,
	path = "get_archive",
	operation_id = "challenge_get_archive",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to get the archive of")
	),
	responses(
		(status = OK, body = Binary, description = "returns a zip archive of the requested challenge (which may be none, in which case the body will be empty)", content_type = "application/zip"),
		(status = NOT_FOUND, description = "challenge with requested id doesn't exist")
	),
	security(
		("user_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn get_archive(State(state): State<AppState>, query: Query<ChallengeByIdQuery>, _user: JWTUser) -> Result<([(header::HeaderName, &'static str); 1], Vec<u8>), StatusCode> {
	use crate::schema::challenges::dsl;

	let mut conn = state.pool().get().await.unwrap();

	match dsl::challenges.find(query.challenge_id).select(dsl::archive).get_result::<Option<Vec<u8>>>(&mut conn).await {
		Ok(archive) => Ok(([(header::CONTENT_TYPE, "application/zip")], archive.unwrap_or(vec![]))),
		Err(diesel::result::Error::NotFound) => {
			Err(StatusCode::NOT_FOUND)
		},
		Err(e) => {
			log::error!("{}", e);
			Err(StatusCode::INTERNAL_SERVER_ERROR)
		}
	}
}

#[utoipa::path(
	post,
	path = "start",
	operation_id = "challenge_start",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to start")
	),
	responses(
		(status = OK, body = String, description = "returns a WireGuard configuration used to connect to the started challenge", content_type = "text/plain"),
		(status = NOT_FOUND, description = "returns a reason for the error", content_type = "text/plain")
	),
	security(
		("user_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn start(State(state): State<AppState>, query: Query<ChallengeByIdQuery>, user: JWTUser) -> Result<String, (StatusCode, String)> {
	use crate::schema::challenges::dsl;

	let mut conn = state.pool().get().await.unwrap();

	match dsl::challenges.find(query.challenge_id).select((dsl::docker_image, dsl::port_numbers, dsl::port_types)).get_result::<(Option<String>, Vec<Option<i16>>, Vec<Option<PortType>>)>(&mut conn).await {
		Ok((Some(image_name), port_nums, port_types)) => {
			// database constraints mean this *should* never fail
			// so lets be loud and annoying if that assumption is ever violated
			assert_eq!(port_nums.len(), port_types.len());
			assert_eq!(port_nums.iter().map(Option::is_some).count(), port_types.iter().map(Option::is_some).count());

			let mappings = port_nums.into_iter().flatten().zip(port_types.into_iter().flatten()).map(|(n, t)| {
				(n as u16, n as u16, t)
			}).collect();

			challd::start_container(&image_name, mappings, user.code).await.map_err(|e| {
				log::error!("{}", e);
				(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
			})
		},
		Ok((None, _port_nums, _port_types)) => {
			Err((StatusCode::NOT_FOUND, String::from("the requested challenge is not configured to be hosted on this server")))
		}
		Err(diesel::result::Error::NotFound) => {
			Err((StatusCode::NOT_FOUND, String::from("challenge with specified id does not exist")))
		},
		Err(e) => {
			log::error!("{}", e);
			Err((StatusCode::INTERNAL_SERVER_ERROR, String::from("something went horribly wrong with the database")))
		}
	}
}

#[derive(Deserialize)]
pub struct ConfigureDockerQuery {
	challenge_id: i32,
	image_name: String
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExposePortType {
	Tcp,
	Udp
}

#[derive(Deserialize, ToSchema)]
pub struct ExposePorts {
	#[schema(example = json!({"80": "tcp"}))]
	ports: HashMap<u16, ExposePortType>
}

/// Idempotently associate a docker image and port numbers with a challenge
#[utoipa::path(
	put,
	path = "configure_docker",
	operation_id = "challenge_configure_docker",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to configure"),
		("image_name" = String, Query, description = "the docker image name to associate with the challenge", example = "nginx")
	),
	request_body(content = ExposePorts, description = "a dictionary which describes the ports (and their types) to associate with the docker image"),
	responses(
		(status = OK, description = "successfully configured docker"),
		(status = BAD_REQUEST, description = "too many port mappings supplied"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action"),
		(status = CONFLICT, description = "challenge with the same docker image already exists"),
		(status = NOT_FOUND, description = "challenge with requested id doesn't exist")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn configure_docker(State(state): State<AppState>, admin: JWTAdmin, query: Query<ConfigureDockerQuery>, Json(port_map): Json<ExposePorts>) -> StatusCode {
	use crate::schema::challenges::dsl;

	if !admin.can_docker {
		return StatusCode::FORBIDDEN;
	}

	let mut port_nums = Vec::with_capacity(port_map.ports.len());
	let mut port_types = Vec::with_capacity(port_map.ports.len());

	for (n, t) in port_map.ports.into_iter() {
		port_nums.push(n as i16);
		port_types.push(match t {
			ExposePortType::Tcp => PortType::Tcp,
			ExposePortType::Udp => PortType::Udp
		})
	}

	let mut conn = state.pool().get().await.unwrap();

	match diesel::update(dsl::challenges).filter(dsl::id.eq(query.challenge_id)).set((
		dsl::docker_image.eq(query.0.image_name),
		dsl::port_numbers.eq(port_nums),
		dsl::port_types.eq(port_types)
	)).execute(&mut conn).await {
		Ok(_) => StatusCode::OK,
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			StatusCode::CONFLICT
		},
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::CheckViolation, _)) => {
			StatusCode::BAD_REQUEST
		},
		Err(diesel::result::Error::NotFound) => {
			StatusCode::NOT_FOUND
		},
		Err(e) => {
			log::error!("{}", e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	}
}

#[utoipa::path(
	get,
	path = "get_flag",
	operation_id = "challenge_get_flag",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to get the flag of")
	),
	responses(
		(status = OK, body = String, description = "returns the flag of the challenge"),
		(status = NOT_FOUND, description = "challenge with requested id doesn't exist, or the challenge has no flag")
	),
	security(
		("user_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn get_flag(State(state): State<AppState>, query: Query<ChallengeByIdQuery>, _user: JWTUser) -> Result<String, StatusCode> {
	use crate::schema::challenges::dsl;

	let mut conn = state.pool().get().await.unwrap();

	let result = dsl::challenges.find(query.challenge_id).select(dsl::flag).get_result(&mut conn).await;

	let flag: Option<String> = match result {
		Ok(challenge) => challenge,
		Err(diesel::result::Error::NotFound) => {
			return Err(StatusCode::NOT_FOUND);
		},
		Err(e) => {
			log::error!("{}", e);
			return Err(StatusCode::INTERNAL_SERVER_ERROR);
		}
	};

	flag.ok_or(StatusCode::NOT_FOUND)
}

// thanks axum 0.8 I really love this new OptionalFromRequest trait its such a great developer experience!
pub struct ConnectionInstructions(String);

impl<S: Sync> OptionalFromRequest<S> for ConnectionInstructions {
	type Rejection = (StatusCode, &'static str);

	async fn from_request(req: Request<Body>, _state: &S) -> Result<Option<Self>, Self::Rejection> {
		let b: Vec<u8> = req.into_body().into_data_stream().map_ok(|b| {
			Vec::from(b)
		}).try_concat().await.map_err(|_| (StatusCode::BAD_REQUEST, "invalid body"))?;

		Ok(match b.is_empty() {
			true => None,
			false => Some(Self(String::from_utf8(b).map_err(|_| (StatusCode::BAD_REQUEST, "non-utf8 string"))?))
		})
	}
}

#[utoipa::path(
	post,
	path = "set_connection_instructions",
	operation_id = "challenge_set_connection_instructions",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to set instructions for")
	),
	request_body(content = Option<String>, description = "the text of the connection instructions"),
	responses(
		(status = OK, description = "successfully set connection instructions"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action"),
		(status = NOT_FOUND, description = "challenge with requested id doesn't exist")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn set_connection_instructions(State(state): State<AppState>, admin: JWTAdmin, query: Query<ChallengeByIdQuery>, instructions: Option<ConnectionInstructions>) -> StatusCode {
	use crate::schema::challenges::dsl;

	if !admin.can_create {
		return StatusCode::FORBIDDEN;
	}

	let mut conn = state.pool().get().await.unwrap();

	match diesel::update(dsl::challenges).filter(dsl::id.eq(query.challenge_id)).set(
		dsl::connection_instructions.eq(instructions.map(|i| i.0))
	).execute(&mut conn).await {
		Ok(_) => StatusCode::OK,
		Err(diesel::result::Error::NotFound) => {
			StatusCode::NOT_FOUND
		},
		Err(e) => {
			log::error!("{}", e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	}
}

#[utoipa::path(
	post,
	path = "host_demo",
	operation_id = "challenge_host_demo",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to start")
	),
	responses(
		(status = OK, description = "the challenge was started successfully"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action"),
		(status = NOT_FOUND, description = "challenge with requested id doesn't exist or cannot be run")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn host_demo(State(state): State<AppState>, query: Query<ChallengeByIdQuery>, admin: JWTAdmin) -> Result<StatusCode, (StatusCode, String)> {
	use crate::schema::challenges::dsl;

	if !admin.can_host_demo {
		return Err((StatusCode::FORBIDDEN, String::from("you do not have permission to perform this action")));
	}

	let mut conn = state.pool().get().await.unwrap();

	match dsl::challenges.find(query.challenge_id).select((dsl::docker_image, dsl::port_numbers, dsl::port_types)).get_result::<(Option<String>, Vec<Option<i16>>, Vec<Option<PortType>>)>(&mut conn).await {
		Ok((Some(image_name), port_nums, port_types)) => {
			// database constraints mean this *should* never fail
			// so lets be loud and annoying if that assumption is ever violated
			assert_eq!(port_nums.len(), port_types.len());
			assert_eq!(port_nums.iter().map(Option::is_some).count(), port_types.iter().map(Option::is_some).count());

			let mappings = port_nums.into_iter().flatten().zip(port_types.into_iter().flatten()).map(|(n, t)| {
				(n as u16, n as u16, t)
			}).collect();

			challd::start_container(&image_name, mappings, challd::StartMode::Demo).await.map_err(|e| {
				log::error!("{}", e);
				(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
			})?;

			Ok(StatusCode::OK)
		},
		Ok((None, _port_nums, _port_types)) => {
			Err((StatusCode::NOT_FOUND, String::from("the requested challenge is not configured to be hosted on this server")))
		}
		Err(diesel::result::Error::NotFound) => {
			Err((StatusCode::NOT_FOUND, String::from("challenge with specified id does not exist")))
		},
		Err(e) => {
			log::error!("{}", e);
			Err((StatusCode::INTERNAL_SERVER_ERROR, String::from("something went horribly wrong with the database")))
		}
	}
}

#[utoipa::path(
	put,
	path = "set_archive",
	operation_id = "challenge_set_archive",
	params(
		("challenge_id" = i32, Query, description = "the challenge id to update the archive of (or to remove if no body is submitted)")
	),
	request_body(content = CreateChallengeFile, description = "a zip file containing all supplied challenge files (not required)", content_type = "multipart/form-data"),
	responses(
		(status = OK, description = "challenge archive successfully updated"),
		(status = BAD_REQUEST, description = "archive not uploaded correctly"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action"),
		(status = NOT_FOUND, description = "category with requested id does not exist")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "challenges"
)]
pub async fn set_archive(State(state): State<AppState>, Query(query): Query<ChallengeByIdQuery>, admin: JWTAdmin, mut multi: Multipart) -> StatusCode {
	use crate::schema::challenges::dsl;

	if !admin.can_create {
		return StatusCode::FORBIDDEN;
	}

	let mut conn = state.pool().get().await.unwrap();

	let archive: Option<Vec<u8>> = match multi.next_field().await {
		Ok(Some(field)) if
			field.name() == Some("zip_file") &&
			field.content_type() == Some("application/zip") =>
		{
			if let Ok(bytes) = field.bytes().await {
				Some(bytes.into())
			} else{
				return StatusCode::BAD_REQUEST;
			}
		},
		Ok(Some(_)) => return StatusCode::BAD_REQUEST,
		Ok(None) => None,
		Err(_) => return StatusCode::BAD_REQUEST
	};

	match diesel::update(dsl::challenges)
		.filter(dsl::id.eq(query.challenge_id))
		.set(dsl::archive.eq(archive))
		.execute(&mut conn).await
	{
		Ok(_) => StatusCode::OK,
		Err(diesel::result::Error::NotFound) => StatusCode::NOT_FOUND,
		Err(e) => {
			log::error!("{}", e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	}
}
