-- Phase 1 MVP schema: PostGIS spatial validation + species catalog + captures.

CREATE EXTENSION IF NOT EXISTS postgis;

CREATE TABLE IF NOT EXISTS species_catalog (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    taxon_source TEXT NOT NULL CHECK (taxon_source IN ('gbif', 'i_naturalist')),
    taxon_external_id BIGINT NOT NULL CHECK (taxon_external_id > 0),
    scientific_name TEXT NOT NULL,
    common_name TEXT,
    phylum TEXT NOT NULL,
    taxonomic_class TEXT NOT NULL,
    average_mass_grams BIGINT NOT NULL CHECK (average_mass_grams >= 1),
    diet TEXT NOT NULL CHECK (diet IN ('carnivore', 'herbivore', 'omnivore', 'detritivore')),
    habitat TEXT NOT NULL CHECK (habitat IN ('abyssal', 'marine', 'freshwater', 'terrestrial', 'aerial')),
    defenses JSONB NOT NULL DEFAULT '[]'::jsonb,
    iucn_status TEXT NOT NULL CHECK (iucn_status IN ('LC', 'NT', 'VU', 'EN', 'CR', 'UNKNOWN')),
    source_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (taxon_source, taxon_external_id)
);

CREATE TABLE IF NOT EXISTS sanctuary_zones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    zone_type TEXT NOT NULL CHECK (zone_type IN ('zoo', 'aquarium', 'uma')),
    boundary geometry(MultiPolygon, 4326) NOT NULL,
    source_attribution TEXT NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sanctuary_zones_boundary
    ON sanctuary_zones USING GIST (boundary);

CREATE TABLE IF NOT EXISTS species_wild_ranges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    taxon_source TEXT NOT NULL CHECK (taxon_source IN ('gbif', 'i_naturalist')),
    taxon_external_id BIGINT NOT NULL CHECK (taxon_external_id > 0),
    range_geom geometry(MultiPolygon, 4326) NOT NULL,
    source_attribution TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_species_wild_ranges_taxon
    ON species_wild_ranges (taxon_source, taxon_external_id);

CREATE INDEX IF NOT EXISTS idx_species_wild_ranges_geom
    ON species_wild_ranges USING GIST (range_geom);

CREATE TABLE IF NOT EXISTS captures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    capture_id TEXT NOT NULL UNIQUE,
    user_id TEXT,
    device_public_key BYTEA NOT NULL,
    image_sha256 BYTEA NOT NULL CHECK (octet_length(image_sha256) = 32),
    captured_at_utc TIMESTAMPTZ NOT NULL,
    location geography(Point, 4326) NOT NULL,
    zone_classification TEXT NOT NULL CHECK (zone_classification IN ('wild', 'sanctuary', 'discarded')),
    sanctuary_zone_id UUID REFERENCES sanctuary_zones(id),
    taxon_source TEXT NOT NULL CHECK (taxon_source IN ('gbif', 'i_naturalist')),
    taxon_external_id BIGINT NOT NULL CHECK (taxon_external_id > 0),
    vision_confidence REAL NOT NULL CHECK (vision_confidence >= 0.0 AND vision_confidence <= 1.0),
    generated_card JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_captures_location
    ON captures USING GIST (location);

CREATE INDEX IF NOT EXISTS idx_captures_user_created
    ON captures (user_id, created_at DESC);
