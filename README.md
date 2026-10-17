# Kokumi

Counts visits to pages, markdown twins and `llms.txt` at Cloudflare's edge, so agents count as visitors, then serves the static site. Rust on Cloudflare Workers (workers-rs) plus D1. MIT.

Slice 1 stores per day, path, `?ref=` tag, referrer host and who asked (person / verified agent / claimed agent / unconfirmed browser / unrecognised). It stores no IP, no user-agent text, no cookie and no id. An agent is verified by its IP being in the operator's published list (`ranges/`, see `ranges/SOURCES.md`), never by ASN.

- `GET /api/v1/counts?days=7` and `GET /api/v1/hits?limit=100`, with `Authorization: Bearer <READ_TOKEN>`.
- `POST /api/hit`: the page beacon (same contract as the old `visits.ts`); a person is counted here.
- `cargo run --example verify` shows how requests are classified.
- Preview: `PATH=~/.local/share/rust-official/bin:$PATH wrangler deploy --config wrangler.preview.jsonc`. Schema: `schema.sql`.
