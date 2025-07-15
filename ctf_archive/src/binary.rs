use utoipa::ToSchema;

#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct Binary(());
