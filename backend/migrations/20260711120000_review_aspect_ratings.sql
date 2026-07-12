-- Reviews rate individual aspects. Bathroom cleanliness stays in `clean`
-- (required, 1-5); coffee and food are optional 1-5 scores a reviewer can
-- add when the place offers them. Per-aspect place ratings are the averages
-- of these columns.
ALTER TABLE reviews
    ADD COLUMN coffee SMALLINT CHECK (coffee BETWEEN 1 AND 5),
    ADD COLUMN food SMALLINT CHECK (food BETWEEN 1 AND 5);
