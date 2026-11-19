-- Phase 1 Almanac extension: adds encyclopedic metadata columns to species_catalog.

ALTER TABLE species_catalog
    ADD COLUMN IF NOT EXISTS summary TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS category TEXT NOT NULL DEFAULT 'general',
    ADD COLUMN IF NOT EXISTS region_hint TEXT,
    ADD COLUMN IF NOT EXISTS is_core_almanac BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_species_catalog_almanac
    ON species_catalog (is_core_almanac, category, scientific_name);
