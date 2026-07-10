# Changelog

All notable changes to this project are logged here as they happen.
Newest entries at the top. Format: `YYYY-MM-DD HH:MM` (local time) —
short description.

## 2026-07-10

- 15:36 — Merged branch `place-model` into `main`.
- 15:32 — Renamed "stop"/"stops" to "place"/"places" throughout the UI,
  API errors, and demo data; replaced business-category place types
  (Coffee, Grocery, Bookstore, Gas, Restroom) with generic venue types
  (Shop, Store, Mall, Public, Hall); added a multi-select amenities field
  (Restrooms, Coffee, Food, Groceries, Seating, Parking); and changed
  "Purchase required?"/"Code required?" from booleans to a three-state
  Yes/No/Don't know `Requirement`. Added migration `0002` to convert
  existing data and a new `amenities` column.
- 13:57 — Added this changelog and updated AGENTS.md to require dated
  entries alongside commits.
