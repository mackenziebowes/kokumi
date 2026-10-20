-- Edge hits: one row per day, path and who asked. No IP, no user-agent text, no cookie, no id.
CREATE TABLE IF NOT EXISTS hits (
  day TEXT NOT NULL, path TEXT NOT NULL, kind TEXT NOT NULL,
  ref TEXT NOT NULL, referrer_host TEXT NOT NULL,
  audience TEXT NOT NULL, operator TEXT NOT NULL, product TEXT NOT NULL,
  n INTEGER NOT NULL,
  PRIMARY KEY (day, path, kind, ref, referrer_host, audience, operator, product)
);
-- Pane's table (JS beacon), kept as it is so the existing rows stay readable.
CREATE TABLE IF NOT EXISTS visits (
  day TEXT NOT NULL, path TEXT NOT NULL, ref TEXT NOT NULL, referrer_host TEXT NOT NULL,
  n INTEGER NOT NULL,
  PRIMARY KEY (day, path, ref, referrer_host)
);
-- Slice 2 (design #1508 rev 6): one row per page view whose script ran. ip, visitor and place stay NULL for
-- EU, EEA and UK visitors. visitor = first 12 bytes of sha256(VISITOR_SALT, ip, user agent), hex.
CREATE TABLE IF NOT EXISTS views (
  view TEXT PRIMARY KEY, day TEXT NOT NULL, at INTEGER NOT NULL, path TEXT NOT NULL,
  ref TEXT NOT NULL, referrer_host TEXT NOT NULL, screen TEXT NOT NULL, language TEXT NOT NULL,
  ip TEXT, visitor TEXT, country TEXT NOT NULL, region TEXT, city TEXT, asn INTEGER, network TEXT,
  visible_ms INTEGER NOT NULL DEFAULT 0, deepest_scroll INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS views_day ON views (day);
CREATE INDEX IF NOT EXISTS views_visitor ON views (visitor);
CREATE TABLE IF NOT EXISTS clicks (view TEXT NOT NULL, at INTEGER NOT NULL, kind TEXT NOT NULL, target TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS clicks_view ON clicks (view);
