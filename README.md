# Kokumi

Counts visits to pages, markdown twins and `llms.txt` at Cloudflare's edge, so agents count as visitors, then serves the static site. Rust on Cloudflare Workers (workers-rs) plus D1. MIT.

Slice 1 stores per day, path, `?ref=` tag, referrer host and who asked (person / verified agent / claimed agent / unconfirmed browser / unrecognised). It stores no IP, no user-agent text, no cookie and no id. An agent is verified by its IP being in the operator's published list (`ranges/`, see `ranges/SOURCES.md`), never by ASN.

- `GET /api/v1/counts?days=7` and `GET /api/v1/hits?limit=100`, with `Authorization: Bearer <READ_TOKEN>`.
- Tools (#2117): `POST /api/php-check` is passed to the `php-check` Worker over the service binding `PHP_CHECK` (src/tool.rs lists the routes). Kokumi rate-limits it per IP (binding `TOOL_LIMIT`), writes one `tool_events` row from the tool's `x-tool-outcome` header (day, tool, the typed public site name, outcome, verdict, Turnstile result, who called, visitor key, country; no IP), and strips the header. The answer returns even if the write fails. `GET /api/v1/tools?days=7&tool=` reads them, with the site names found below the fix.
- `POST /api/hit`: the page beacon (same contract as the old `visits.ts`); a person is counted here.
- `cargo run --example verify` shows how requests are classified.
- Preview: `PATH=~/.local/share/rust-official/bin:$PATH wrangler deploy --config wrangler.preview.jsonc`. Schema: `schema.sql`.
