CREATE TABLE categories (
	id SERIAL PRIMARY KEY,
	name VARCHAR(63) UNIQUE NOT NULL,
	ctf_id SERIAL REFERENCES ctfs(id) ON DELETE CASCADE,
	category_type VARCHAR(63) NOT NULL CONSTRAINT valid_category_type CHECK (
		category_type = 'rev' OR
		category_type = 'pwn' OR
		category_type = 'web' OR
		category_type = 'crypto' OR
		category_type = 'stego' OR
		category_type = 'forensics' OR
		category_type = 'misc'),
	CONSTRAINT unique_name_per_ctf UNIQUE (name, ctf_id)
);
