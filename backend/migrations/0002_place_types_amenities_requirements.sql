-- "Type of place" moves from business categories to generic venue kinds.
UPDATE places SET place_type = CASE place_type
    WHEN 'coffee' THEN 'shop'
    WHEN 'bookstore' THEN 'shop'
    WHEN 'grocery' THEN 'store'
    WHEN 'gas' THEN 'store'
    WHEN 'restroom' THEN 'public'
    ELSE place_type
END;

-- purchase_required / code_required become three-state: yes, no, unknown.
ALTER TABLE places
    ALTER COLUMN purchase_required DROP DEFAULT,
    ALTER COLUMN purchase_required TYPE TEXT
        USING (CASE WHEN purchase_required THEN 'yes' ELSE 'no' END),
    ALTER COLUMN purchase_required SET DEFAULT 'unknown',
    ALTER COLUMN code_required DROP DEFAULT,
    ALTER COLUMN code_required TYPE TEXT
        USING (CASE WHEN code_required THEN 'yes' ELSE 'no' END),
    ALTER COLUMN code_required SET DEFAULT 'unknown';

-- Amenities: multi-select tags for what the place has on offer.
ALTER TABLE places ADD COLUMN amenities TEXT[] NOT NULL DEFAULT '{}';
