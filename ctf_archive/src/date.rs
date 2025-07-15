use serde::{Serialize, Deserialize};
use utoipa::ToSchema;
use chrono::{NaiveDate, Datelike};

// I know this isn't very robust, but it's simple, and imo that's more important
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Date {
	#[schema(example = 2020)]
	year: u16,
	#[schema(example = 1)]
	month: u8,
	#[schema(example = 1)]
	day: u8
}

impl std::convert::TryFrom<NaiveDate> for Date {
	type Error = ();

	fn try_from(date: NaiveDate) -> Result<Self, Self::Error> {
		let year: u16 = date.year().try_into().map_err(|_| ())?;

		Ok(Self {
			year,
			month: date.month() as u8,
			day: date.day() as u8
		})
	}
}

impl std::convert::TryInto<NaiveDate> for Date {
	type Error = ();

	fn try_into(self) -> Result<NaiveDate, Self::Error> {
		NaiveDate::from_ymd_opt(self.year as i32, self.month as u32, self.day as u32).ok_or(())
	}
}
