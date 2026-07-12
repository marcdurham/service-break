-- Demo data from the design mockup, placed around downtown Seattle.
-- Applied at startup only when the places table is empty (see db::ensure_seeded).

INSERT INTO places (id, name, place_type, lat, lng, address, door_ft, door_note, parking, purchase_required, code_required, amenities, hours, device_id) VALUES
('00000000-0000-0000-0000-000000000001', 'Camber Coffee',        'shop',  47.6117, -122.3402, '214 Maple Ave',        15, 'Right past the counter, first door on the left', 'street', 'yes', 'yes', '{restrooms,coffee,seating}',    'Open · closes 8:00 PM',  'seed'),
('00000000-0000-0000-0000-000000000002', 'Riverside Grocery',    'store', 47.6097, -122.3508, '88 River Rd',          90, 'Back-left corner past produce',                  'easy',   'no',  'no',  '{restrooms,groceries,parking}', 'Open · closes 10:00 PM', 'seed'),
('00000000-0000-0000-0000-000000000003', 'Chapter & Verse Books','shop',  47.6184, -122.3402, '12 Oak St',            40, 'Upstairs by the cafe',                            'street', 'no',  'yes', '{restrooms,coffee,seating}',    'Open · closes 9:00 PM',  'seed'),
('00000000-0000-0000-0000-000000000004', 'QuikFuel Station',     'store', 47.6046, -122.3487, '400 Highway 9',        25, 'Around the side of the building',                 'easy',   'no',  'no',  '{restrooms,food,parking}',      'Open 24 hours',          'seed'),
('00000000-0000-0000-0000-000000000005', 'Elm Street Park',      'park',  47.6062, -122.3387, 'Elm St & 5th',          0, 'Restroom building by the trailhead',              'easy',   'no',  'no',  '{restrooms,parking,seating}',   'Open · dawn to dusk',    'seed'),
('00000000-0000-0000-0000-000000000006', 'City Center Restroom', 'public',47.6198, -122.3422, 'Center Plaza',          0, 'Standalone public facility on the plaza',         'none',   'no',  'no',  '{restrooms}',                   'Open · 6 AM–11 PM',      'seed'),
('00000000-0000-0000-0000-000000000007', 'Wildflour Bakery',     'shop',  47.6097, -122.3228, '55 Birch Ln',          10, 'Immediately right of the entrance',               'street', 'yes', 'yes', '{restrooms,coffee,food,seating}','Open · closes 6:00 PM',  'seed'),
('00000000-0000-0000-0000-000000000008', 'Northgate Market',     'store', 47.6210, -122.3550, '900 North Gate Blvd',  70, 'Near the pharmacy counter',                       'easy',   'no',  'no',  '{restrooms,groceries,parking}', 'Open · closes 11:00 PM', 'seed');

INSERT INTO reviews (place_id, device_id, clean, coffee, food, text, created_at) VALUES
('00000000-0000-0000-0000-000000000001', 'PriyaM99', 5, 5,    NULL, 'Spotless. Single room, key on a wooden spoon by the register. Worth the visit.', now() - interval '2 days'),
('00000000-0000-0000-0000-000000000001', 'DevR4321', 4, 4,    NULL, 'Clean and quick. Small line at peak but the code is on the receipt.',           now() - interval '7 days'),
('00000000-0000-0000-0000-000000000002', 'SamK7710', 4, NULL, NULL, 'No purchase needed, family restroom is big and clean. Long walk from the door though.', now() - interval '4 days'),
('00000000-0000-0000-0000-000000000003', 'LenaT550', 5, 4,    NULL, 'Cozy and clean. They give you the code at the counter, no purchase required.',  now() - interval '6 days'),
('00000000-0000-0000-0000-000000000004', 'MarcoB12', 3, NULL, 2,    'It is a gas station bathroom. Functional, key at the register. Bring your own wipes.', now() - interval '3 days'),
('00000000-0000-0000-0000-000000000005', 'NadiaF88', 4, NULL, NULL, 'Surprisingly well kept for a public park. Baby changing table too.',            now() - interval '1 day'),
('00000000-0000-0000-0000-000000000006', 'OwenP333', 3, NULL, NULL, 'Free and central. Cleaned a few times a day, hit or miss depending on time.',   now() - interval '5 days'),
('00000000-0000-0000-0000-000000000007', 'TaraW202', 5, 5,    5,    'Cleanest bathroom on this whole app. Fresh flowers, always stocked.',           now() - interval '2 days'),
('00000000-0000-0000-0000-000000000008', 'IrisH505', 4, NULL, NULL, 'Reliable and clean, no purchase needed. Lot parking is easy.',                  now() - interval '7 days');
