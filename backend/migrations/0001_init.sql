CREATE TABLE places (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    place_type TEXT NOT NULL,
    lat DOUBLE PRECISION NOT NULL,
    lng DOUBLE PRECISION NOT NULL,
    address TEXT NOT NULL DEFAULT '',
    door_ft INTEGER NOT NULL DEFAULT 0,
    door_note TEXT NOT NULL DEFAULT '',
    parking TEXT NOT NULL DEFAULT 'street',
    purchase_required BOOLEAN NOT NULL DEFAULT FALSE,
    code_required BOOLEAN NOT NULL DEFAULT FALSE,
    hours TEXT,
    device_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    place_id UUID NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    clean SMALLINT NOT NULL CHECK (clean BETWEEN 1 AND 5),
    text TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE saved_places (
    device_id TEXT NOT NULL,
    place_id UUID NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, place_id)
);

CREATE INDEX idx_places_type ON places (place_type);
CREATE INDEX idx_reviews_place ON reviews (place_id);
