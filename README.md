# Kokumi

Counts visits to pages, markdown twins and `llms.txt` at Cloudflare's edge, so agents count as visitors, then serves the static site. Rust on Cloudflare Workers (workers-rs) plus D1. MIT.

Slice 1 stores per day, path, `?ref=` tag, referrer host and who asked (person / verified agent / claimed agent / unconfirmed browser / unrecognised). It stores no IP, no user-agent text, no cookie and no id. An agent is verified by its IP being in the operator's published list (`ranges/`, see `ranges/SOURCES.md`), never by ASN.

Internal clicks (#4286): a counted page request whose Referer is another page of the same site (with or without www.) also adds one to `internal_hits` for that day, the page and the page it came from. The source is a path on our own site: no host, no query, no fragment, at most 200 characters; nothing about the person. Referers from other sites are still host only, and an agent or a browser that sends no Referer adds nothing. `GET /api/v1/internal?days=N` (read token) returns the rows, `truncated` says when the 50,000-row cap cut them. A deploy needs the new table first: `wrangler d1 execute <database> --remote --file schema.sql` (every statement is `IF NOT EXISTS`).

- `GET /api/v1/counts?days=7` and `GET /api/v1/hits?limit=100`, with `Authorization: Bearer <READ_TOKEN>`.
- `GET /api/v1/views?day=YYYY-MM-DD` (or `?days=N`) answers `truncated: true` when it hit its 5,000-row cap. `GET /api/v1/links?day=YYYY-MM-DD` (#4322) gives each view's seen and clicked events on links to this site's pages, one row per view, component and area, from `component_events`; nothing new is collected.
- Tools (#2117): `POST /api/php-check` is passed to the `php-check` Worker over the service binding `PHP_CHECK` (src/tool.rs lists the routes). Kokumi rate-limits it per IP (binding `TOOL_LIMIT`), writes one `tool_events` row from the tool's `x-tool-outcome` header (day, tool, the typed public site name, outcome, verdict, Turnstile result, who called, visitor key, country; no IP), and strips the header. The answer returns even if the write fails. `GET /api/v1/tools?days=7&tool=` reads them, with the site names found below the fix.
- `POST /api/hit`: the page beacon (same contract as the old `visits.ts`); a person is counted here.
- `cargo run --example verify` shows how requests are classified.
- Preview: `PATH=~/.local/share/rust-official/bin:$PATH wrangler deploy --config wrangler.preview.jsonc`. Schema: `schema.sql`.
