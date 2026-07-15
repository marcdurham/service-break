# Browser Login Instructions

Quick reference for logging into the Service Break app via browser automation.

## Prerequisites

1. App running on port 8883 (or configured port)
2. Chrome with remote debugging enabled (`--remote-debugging-port=9222`)
3. Puppeteer installed in `browser-tools/` directory

## Admin Credentials

- **Username:** `admin`
- **Password:** `I brake for coffee`
- Set automatically by backend on first run via `backend/src/admin.rs`

## Quick Login Script

```javascript
// /tmp/type_and_click.js
import puppeteer from "puppeteer-core";

const b = await Promise.race([
    puppeteer.connect({
        browserURL: "http://localhost:9222",
        defaultViewport: null,
    }),
    new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 5000)),
]).catch((e) => {
    console.error("✗ Could not connect to browser:", e.message);
    process.exit(1);
});

const p = (await b.pages()).at(-1);

// Type credentials
await p.type('input[placeholder*="trail-scout"]', "admin");
await p.type('input[type="password"]', "I brake for coffee");

// Click sign in
await p.click(".submit-btn");
console.log("Logged in!");

await b.disconnect();
```

Run with: `cd browser-tools && node /tmp/type_and_click.js`

## Navigation Tips

- **Onboarding page:** Set `localStorage.setItem("sb_started", JSON.stringify(true))` to skip, or click "Enable location & explore" button
- **Account page:** `/account` — shows login form when not authenticated
- **Admin panel:** `/admin` — accessible after logging in as admin user

## Common Issues

1. **Chrome not running:** Start with `google-chrome --remote-debugging-port=9222 &`
2. **Yew inputs won't accept values:** Use `puppeteer.type()` instead of setting `.value` directly (Yew uses controlled components)
3. **Onboarding stuck:** The app checks `sb_started` as a boolean in localStorage, not string "true"

## Admin Panel Features

- View all users (`/users`)
- Export full backup (JSON, excludes password hashes)
- Import backup (replaces current data)
