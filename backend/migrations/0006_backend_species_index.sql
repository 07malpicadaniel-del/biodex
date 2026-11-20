-- Separate deep backend taxonomic catalog index from lightweight UI collection display
ALTER TABLE species_catalog
    ADD COLUMN IF NOT EXISTS is_backend_indexed BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE species_catalog
SET is_backend_indexed = TRUE
WHERE is_core_almanac = TRUE OR is_naturalista_synced = TRUE;

CREATE INDEX IF NOT EXISTS idx_species_catalog_scientific_lower
    ON species_catalog (LOWER(scientific_name));

CREATE INDEX IF NOT EXISTS idx_species_catalog_backend_index
    ON species_catalog (is_core_almanac, is_naturalista_synced, is_backend_indexed, naturalista_observations_count DESC NULLS LAST);
