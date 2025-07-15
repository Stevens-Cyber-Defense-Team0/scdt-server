// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "port_type"))]
    pub struct PortType;
}

diesel::table! {
    categories (id) {
        id -> Int4,
        #[max_length = 63]
        name -> Varchar,
        ctf_id -> Int4,
        #[max_length = 63]
        category_type -> Varchar,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PortType;

    challenges (id) {
        id -> Int4,
        #[max_length = 63]
        name -> Varchar,
        #[max_length = 127]
        flag -> Nullable<Varchar>,
        category_id -> Int4,
        #[max_length = 2047]
        description -> Varchar,
        #[max_length = 31]
        difficulty -> Varchar,
        archive -> Nullable<Bytea>,
        #[max_length = 63]
        docker_image -> Nullable<Varchar>,
        port_numbers -> Array<Nullable<Int2>>,
        port_types -> Array<Nullable<PortType>>,
        #[max_length = 255]
        connection_instructions -> Nullable<Varchar>,
    }
}

diesel::table! {
    ctfs (id) {
        id -> Int4,
        #[max_length = 127]
        name -> Varchar,
        start_date -> Date,
        end_date -> Date,
    }
}

diesel::joinable!(categories -> ctfs (ctf_id));
diesel::joinable!(challenges -> categories (category_id));

diesel::allow_tables_to_appear_in_same_query!(
    categories,
    challenges,
    ctfs,
);
