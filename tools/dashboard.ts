// A crude local Kokumi dashboard: reads the live read API and writes one static HTML page.
//   bun tools/dashboard.ts [days=7] [out=/tmp/kokumi-dashboard.html]
// The token stays on this machine (~/.config/kokumi/read-token); the page holds only counts.
const days = Number(Bun.argv[2] ?? 7);
const out = Bun.argv[3] ?? "/tmp/kokumi-dashboard.html";
const token = (await Bun.file(`${process.env.HOME}/.config/kokumi/read-token`).text()).trim();
const site = "https://www.mackenziebowes.com";
const get = async (p: string) => {
  const r = await fetch(site + p, { headers: { authorization: `Bearer ${token}` } });
  if (!r.ok) throw new Error(`${p}: ${r.status}`);
  return r.json() as Promise<any>;
};
type Hit = { day: string; path: string; kind: string; ref: string; referrer_host: string; audience: string; operator: string; product: string; n: number };
const hits: Hit[] = (await get(`/api/v1/hits?limit=10000`)).hits;
const counts = await get(`/api/v1/counts?days=${days}`);
const since = new Date(Date.now() - days * 864e5).toISOString().slice(0, 10);
const rows = hits.filter((h) => h.day >= since);

const who = (h: Hit) => (h.audience.includes("agent") ? "agent" : h.audience === "unconfirmed-browser" ? "browser" : "other");
const sum = (xs: Hit[]) => xs.reduce((a, h) => a + h.n, 0);
const group = (xs: Hit[], key: (h: Hit) => string) => {
  const m = new Map<string, number>();
  for (const h of xs) m.set(key(h), (m.get(key(h)) ?? 0) + h.n);
  return [...m].sort((a, b) => b[1] - a[1]);
};
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const table = (title: string, head: string[], body: (string | number)[][], note = "") =>
  `<section><h2>${title}</h2>${note ? `<p class="note">${note}</p>` : ""}<table><tr>${head.map((h, i) => `<th${i ? ' class="n"' : ""}>${h}</th>`).join("")}</tr>${
    body.length ? body.map((r) => `<tr>${r.map((c, i) => `<td${i ? ' class="n"' : ""}>${esc(String(c))}</td>`).join("")}</tr>`).join("") : `<tr><td colspan="${head.length}" class="note">none yet</td></tr>`
  }</table></section>`;

const dayList = [...new Set(rows.map((h) => h.day))].sort();
const byDay = dayList.map((d) => {
  const xs = rows.filter((h) => h.day === d);
  return [d, sum(xs.filter((h) => who(h) === "browser")), sum(xs.filter((h) => who(h) === "agent")), sum(xs.filter((h) => who(h) === "other"))];
});
const pages = group(rows.filter((h) => h.kind === "page"), (h) => h.path).slice(0, 25).map(([p]) => {
  const xs = rows.filter((h) => h.path === p);
  const beacon = (counts.beacon as any[]).filter((b) => b.path === p).reduce((a, b) => a + b.n, 0);
  const twin = sum(rows.filter((h) => h.path === p + ".md"));
  return [p, sum(xs.filter((h) => who(h) === "browser")), beacon, sum(xs.filter((h) => who(h) === "agent")), twin];
});
const refs = group(rows.filter((h) => h.ref), (h) => `${h.ref}\u0000${h.path}`).map(([k, n]) => [...k.split("\u0000"), n]);
const referrers = group(rows.filter((h) => h.referrer_host), (h) => h.referrer_host);
const agents = group(rows.filter((h) => who(h) === "agent"), (h) => `${h.operator} ${h.product}\u0000${h.audience}`).map(([k, n]) => [...k.split("\u0000"), n]);
const agentPaths = group(rows.filter((h) => who(h) === "agent"), (h) => h.path).slice(0, 15);

const total = sum(rows);
const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Kokumi dashboard</title><style>
:root{--bg:#fbfaf7;--fg:#1d1c1a;--muted:#6b6862;--line:#e3e0d8;--accent:#2f6f5e}
@media (prefers-color-scheme:dark){:root{--bg:#141413;--fg:#ecebe6;--muted:#9a978f;--line:#2c2b28;--accent:#7cc4ad}}
body{background:var(--bg);color:var(--fg);font:15px/1.45 system-ui,sans-serif;margin:0 auto;max-width:980px;padding:24px 16px}
h1{font-size:22px;margin:0 0 4px}h2{font-size:15px;margin:28px 0 8px;color:var(--accent)}
.note{color:var(--muted);font-size:13px;margin:0 0 8px}
.stats{display:flex;gap:28px;flex-wrap:wrap;margin:16px 0}.stats b{display:block;font-size:26px}
table{border-collapse:collapse;width:100%;font-size:14px}td,th{border-bottom:1px solid var(--line);padding:5px 8px;text-align:left;overflow-wrap:anywhere}
.n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}th{color:var(--muted);font-weight:500}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:0 32px}</style></head><body>
<h1>mackenziebowes.com, last ${days} days</h1>
<p class="note">From Kokumi's read API, ${new Date().toLocaleString()}. Browser = a browser user agent (may be a crawler). Script ran = the page's JavaScript reported in, so almost surely a person. Agent = named from its user agent; "verified" only when its IP is on the operator's published list.</p>
<div class="stats"><div><b>${total}</b>requests counted</div><div><b>${sum(rows.filter((h) => who(h) === "browser"))}</b>browser</div><div><b>${(counts.beacon as any[]).reduce((a, b) => a + b.n, 0)}</b>script ran</div><div><b>${sum(rows.filter((h) => who(h) === "agent"))}</b>agent</div></div>
${table("By day", ["Day", "Browser", "Agent", "Other"], byDay.reverse())}
${table("Pages", ["Page", "Browser", "Script ran", "Agent", ".md twin"], pages, "Browser hits with no 'script ran' are probably crawlers.")}
<div class="grid">
${table("?ref= links", ["Tag", "Page", "Hits"], refs)}
${table("Referring sites", ["Site", "Hits"], referrers)}
${table("Agents", ["Agent", "Status", "Hits"], agents)}
${table("What agents read", ["Path", "Hits"], agentPaths)}
</div></body></html>`;
await Bun.write(out, html);
console.log(out);
