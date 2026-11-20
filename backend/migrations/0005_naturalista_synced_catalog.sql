-- Connects live NaturaLista/iNaturalist species sync into species_catalog while strictly forbidding subspecies.

ALTER TABLE species_catalog
    ADD COLUMN IF NOT EXISTS taxonomic_rank TEXT NOT NULL DEFAULT 'species' CHECK (taxonomic_rank = 'species'),
    ADD COLUMN IF NOT EXISTS is_naturalista_synced BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_species_catalog_collection
    ON species_catalog (is_core_almanac DESC, is_naturalista_synced, category, naturalista_observations_count DESC NULLS LAST);
