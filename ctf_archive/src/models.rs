use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use chrono::NaiveDate;
use crate::schema;

#[derive(Insertable)]
#[diesel(table_name = schema::ctfs)]
pub struct NewCtf {
	pub name: String,
	pub start_date: NaiveDate,
	pub end_date: NaiveDate
}

#[derive(Insertable)]
#[diesel(table_name = schema::categories)]
pub struct NewCategory<'a> {
	pub name: String,
	pub ctf_id: i32,
	pub category_type: &'a str
}

#[derive(Debug, PartialEq, DbEnum)]
#[ExistingTypePath = "crate::schema::sql_types::PortType"]
pub enum PortType {
	Tcp,
	Udp
}

#[derive(Insertable)]
#[diesel(table_name = schema::challenges)]
pub struct NewChallenge {
	pub name: String,
	pub flag: Option<String>,
	pub category_id: i32,
	pub description: String,
	pub difficulty: String,
	pub archive: Option<Vec<u8>>,

	// TODO: migrate away from SQL arrays
	// in hindsight, I deeply regret this decision
	pub port_numbers: Vec<i16>,
	pub port_types: Vec<PortType>,

	pub connection_instructions: Option<String>
}
