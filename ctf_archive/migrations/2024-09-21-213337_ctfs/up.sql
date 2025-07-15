CREATE TABLE ctfs (
	id SERIAL PRIMARY KEY,
	name VARCHAR(127) UNIQUE NOT NULL,
	start_date DATE NOT NULL,
	end_date DATE NOT NULL CONSTRAINT ends_after_start CHECK (end_date >= start_date)
);
