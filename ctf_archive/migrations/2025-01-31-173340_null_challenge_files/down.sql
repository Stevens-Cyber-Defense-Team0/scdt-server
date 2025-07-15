ALTER TABLE challenges DROP COLUMN has_archive;
ALTER TABLE challenges ALTER COLUMN archive SET NOT NULL;
