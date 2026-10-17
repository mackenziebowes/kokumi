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
