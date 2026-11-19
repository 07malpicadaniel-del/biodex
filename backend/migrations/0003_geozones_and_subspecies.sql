-- Phase 1 closure: Overpass geo_zones table + subspecies_label provenance on captures.

CREATE TABLE IF NOT EXISTS geo_zones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    osm_type TEXT NOT NULL CHECK (osm_type IN ('way', 'relation')),
    osm_id BIGINT NOT NULL CHECK (osm_id > 0),
    name TEXT NOT NULL,
    zone_category TEXT NOT NULL CHECK (zone_category IN ('sanctuary', 'nature_reserve')),
    zone_type TEXT NOT NULL CHECK (zone_type IN ('zoo', 'aquarium', 'uma', 'nature_reserve')),
    boundary geometry(MultiPolygon, 4326) NOT NULL,
    region_tag TEXT NOT NULL DEFAULT 'veracruz',
    source_attribution TEXT NOT NULL DEFAULT 'OpenStreetMap / Overpass',
    verified BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (osm_type, osm_id)
);

CREATE INDEX IF NOT EXISTS idx_geo_zones_boundary
    ON geo_zones USING GIST (boundary);

CREATE INDEX IF NOT EXISTS idx_geo_zones_category_verified
    ON geo_zones (zone_category, verified);

ALTER TABLE captures
    ADD COLUMN IF NOT EXISTS subspecies_label TEXT,
    ADD COLUMN IF NOT EXISTS geo_zone_id UUID REFERENCES geo_zones(id);
