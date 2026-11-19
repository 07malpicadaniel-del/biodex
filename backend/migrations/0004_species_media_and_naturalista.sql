-- Phase 1 Collection + NaturaLista enrichment: stores cached wildlife imagery & encyclopedia metadata.

ALTER TABLE species_catalog
    ADD COLUMN IF NOT EXISTS image_url TEXT,
    ADD COLUMN IF NOT EXISTS photo_attribution TEXT,
    ADD COLUMN IF NOT EXISTS wikipedia_url TEXT,
    ADD COLUMN IF NOT EXISTS naturalista_observations_count BIGINT CHECK (naturalista_observations_count >= 0);
