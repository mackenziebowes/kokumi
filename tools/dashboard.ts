// Kokumi's local dashboard: reads the live read API, builds an IntUI tree (intui #1957) and writes one
// self-contained HTML page drawn by IntUI's web renderer.
//   bun tools/dashboard.ts [days=7] [out=/tmp/kokumi-dashboard.html]
// The token stays on this machine (~/.config/kokumi/read-token); the page holds only counts.
import { join } from "node:path";
import { Tree } from "../../intui/packages/core/src/index";

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
// The read API returns at most 10,000 hits; when it hands back that many, every figure is a floor, said as one.
const capped = hits.length >= 10000;
const reading = (n: number) => (capped ? { kind: "atLeast", value: n } : n);
// Day buckets are Vancouver days (61a7cff), so today is today in Vancouver; it is a day so far, not a drop.
const today = new Intl.DateTimeFormat("en-CA", { timeZone: "America/Vancouver" }).format(new Date());
const point = (day: string, n: number) => ({ x: day, y: reading(n), ...(day === today ? { period: "partial" } : {}) });

type Cell = string | number;
const table = (caption: string, head: string[], body: Cell[][]) => ({
  type: "Table",
  caption,
  columns: head.map((label, i) => ({ key: `c${i}`, label })),
  rows: body.map((r) => Object.fromEntries(r.map((c, i) => [`c${i}`, c]))),
});
const stat = (label: string, n: number, note?: string) => ({ type: "Stat", label, value: reading(n), tone: "neutral", ...(note ? { note } : {}) });

const root = {
  type: "Group",
  direction: "column",
  children: [
    {
      type: "Text",
      markdown: `# mackenziebowes.com, last ${days} days\n\nFrom Kokumi's read API, ${new Date().toLocaleString()}. Browser = a browser user agent (may be a crawler). Script ran = the page's JavaScript reported in, so almost surely a person. Agent = named from its user agent; "verified" only when its IP is on the operator's published list.${capped ? " The API returned its limit of 10,000 hits, so every figure is at least what it shows." : ""}`,
    },
    {
      type: "Group",
      direction: "row",
      children: [
        stat("Requests counted", total),
        stat("Browser", sum(rows.filter((h) => who(h) === "browser")), "may include crawlers"),
        stat("Script ran", (counts.beacon as any[]).reduce((a, b) => a + b.n, 0), "almost surely a person"),
        stat("Agent", sum(rows.filter((h) => who(h) === "agent"))),
      ],
    },
    {
      type: "Chart",
      kind: "bar",
      title: "Requests by day",
      unit: "requests",
      series: [
        { label: "Browser", role: "primary", points: byDay.map(([d, b]) => point(d as string, b as number)) },
        { label: "Agent", role: "comparison", points: byDay.map(([d, , a]) => point(d as string, a as number)) },
      ],
      annotations: [],
    },
    table("By day", ["Day", "Browser", "Agent", "Other"], [...byDay].reverse()),
    table("Pages (browser hits with no 'script ran' are probably crawlers)", ["Page", "Browser", "Script ran", "Agent", ".md twin"], pages),
    { type: "Group", direction: "row", children: [table("?ref= links", ["Tag", "Page", "Hits"], refs), table("Referring sites", ["Site", "Hits"], referrers)] },
    { type: "Group", direction: "row", children: [table("Agents", ["Agent", "Status", "Hits"], agents), table("What agents read", ["Path", "Hits"], agentPaths)] },
  ],
};

// Checked with core before anything is written, so a bad tree fails here with every problem listed.
const checked = Tree.parse({ v: 1, root }, { author: "code" });
if (checked.kind === "refused") throw new Error(checked.message);

const built = await Bun.build({ entrypoints: [join(import.meta.dir, "dashboard-page.ts")], target: "browser", format: "iife", minify: true });
if (!built.success) throw new Error(built.logs.map(String).join("\n"));
const js = (await built.outputs[0]!.text()).replaceAll("</script", "<\\/script");
const intui = join(import.meta.dir, "../../intui/packages/web/src");
const css = (await Bun.file(join(intui, "style.css")).text()) + (await Bun.file(join(intui, "marks.css")).text()) + (await Bun.file(join(intui, "page.css")).text());
const tree = JSON.stringify(Tree.toJSON(checked.value)).replaceAll("<", "\\u003c");
await Bun.write(
  out,
  `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Kokumi dashboard</title><style>${css}</style></head>
<body><script>window.KOKUMI_TREE=${tree}</script><script>${js}</script></body></html>`,
);
console.log(out);
