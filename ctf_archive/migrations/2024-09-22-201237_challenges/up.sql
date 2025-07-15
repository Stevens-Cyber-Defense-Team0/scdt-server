CREATE TYPE port_type AS ENUM ('tcp', 'udp');

CREATE TABLE challenges (
	id SERIAL PRIMARY KEY,
	name VARCHAR(63) NOT NULL,
	flag VARCHAR(127),
	category_id SERIAL REFERENCES categories(id) ON DELETE RESTRICT,
	description VARCHAR(2047) NOT NULL,
	difficulty VARCHAR(31) NOT NULL,
	archive BYTEA NOT NULL,
	docker_image VARCHAR(63) UNIQUE,
	port_numbers smallint[] NOT NULL,
	port_types port_type[] NOT NULL,
	CONSTRAINT unique_name_per_category UNIQUE (name, category_id),
	CONSTRAINT limited_port_number CHECK (array_length(port_numbers, 0) < 10),
	CONSTRAINT has_port_types CHECK (array_length(port_types, 0) = array_length(port_numbers, 0))
);
