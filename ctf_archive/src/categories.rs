use serde::{Serialize, Deserialize};
use axum::{Json, extract::{State, Query}, http::StatusCode};
use utoipa::{OpenApi, ToSchema};
use diesel::{QueryDsl, ExpressionMethods};
use diesel::result::DatabaseErrorKind;
use diesel_async::RunQueryDsl;
use crate::models::NewCategory;
use crate::state::AppState;
use crate::jwt::{JWTUser, JWTAdmin};

#[derive(OpenApi)]
#[openapi(
	paths(get, create),
	components(schemas(Category, CategoryType, CreateCategory))
)]
pub struct CategoriesApi;

#[derive(Serialize, ToSchema)]
pub struct Category {
	id: i32,
	#[schema(example = "reversing")]
	name: String,
	category_type: CategoryType
}

#[derive(Serialize, Deserialize, ToSchema)]
#[schema(example = "rev")]
pub enum CategoryType {
	#[serde(rename = "rev")]
	Reversing,
	#[serde(rename = "pwn")]
	Exploitation,
	#[serde(rename = "web")]
	Web,
	#[serde(rename = "crypto")]
	Cryptography,
	#[serde(rename = "stego")]
	Stegonography,
	#[serde(rename = "forensics")]
	Forensics,
	#[serde(rename = "misc")]
	Miscellaneous
}

// man... I sure do wish I didn't have to write this boilerplate (cough cough serde)
impl CategoryType {
	fn parse(t: &str) -> Option<Self> {
		match t {
			"rev" => Some(Self::Reversing),
			"pwn" => Some(Self::Exploitation),
			"web" => Some(Self::Web),
			"crypto" => Some(Self::Cryptography),
			"stego" => Some(Self::Stegonography),
			"forensics" => Some(Self::Forensics),
			"misc" => Some(Self::Miscellaneous),
			_ => None
		}
	}

	fn as_str(&self) -> &'static str {
		match self {
			Self::Reversing => "rev",
			Self::Exploitation => "pwn",
			Self::Web => "web",
			Self::Cryptography => "crypto",
			Self::Stegonography => "stego",
			Self::Forensics => "forensics",
			Self::Miscellaneous => "misc"
		}
	}
}

#[derive(Deserialize)]
pub struct GetCategoryQuery {
	ctf_id: i32
}

#[utoipa::path(
	get,
	path = "get",
	operation_id = "category_get",
	params(
		("ctf_id" = i32, Query, description = "the CTF id to find categories of")
	),
	responses(
		(status = OK, body = [Category], description = "returns a list of categories in this CTF, if the CTF doesn't exist `[]` will be returned"),
	),
	security(
		("user_api_key" = [])
	),
	tag = "categories"
)]
pub async fn get(State(state): State<AppState>, query: Query<GetCategoryQuery>, _user: JWTUser) -> Result<Json<Vec<Category>>, StatusCode> {
	use crate::schema::categories::dsl;

	let mut conn = state.pool().get().await.unwrap();

	let result = dsl::categories.select((dsl::id, dsl::name, dsl::category_type)).filter(dsl::ctf_id.eq(query.ctf_id)).get_results(&mut conn).await;

	let categories: Vec<(i32, String, String)> = match result {
		Ok(categories) => categories,
		Err(e) => {
			log::error!("{}", e);
			return Err(StatusCode::INTERNAL_SERVER_ERROR)
		}
	};

	Ok(Json(categories.into_iter().map(|(id, name, category_type)| {
		Category {
			id,
			name,
			category_type: CategoryType::parse(&category_type).expect("db constraints should guarentee this never panics")
		}
	}).collect()))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateCategory {
	#[schema(example = "reversing")]
	name: String,
	ctf_id: i32,
	category_type: CategoryType
}

#[utoipa::path(
	post,
	path = "create",
	operation_id = "category_create",
	request_body = CreateCategory,
	responses(
		(status = CREATED, body = i32, description = "category was added successfully and its id is returned", content_type = "application/json"),
		(status = FORBIDDEN, description = "you do not have permission to perform this action"),
		(status = CONFLICT, description = "category with the same name already exists in this CTF"),
		(status = NOT_FOUND, description = "CTF with requested id does not exist")
	),
	security(
		("admin_api_key" = [])
	),
	tag = "categories"
)]
pub async fn create(State(state): State<AppState>, admin: JWTAdmin, Json(category): Json<CreateCategory>) -> Result<(StatusCode, Json<i32>), StatusCode> {
	use crate::schema::categories::dsl;

	if !admin.can_create {
		return Err(StatusCode::FORBIDDEN);
	}

	let mut conn = state.pool().get().await.unwrap();

	let result = diesel::insert_into(dsl::categories).values(NewCategory {
		name: category.name,
		ctf_id: category.ctf_id,
		category_type: category.category_type.as_str()
	}).returning(dsl::id).get_result(&mut conn).await;

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
