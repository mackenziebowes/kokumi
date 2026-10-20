//! Kokumi: counts visits to pages, markdown twins and llms.txt at the edge, then serves the
//! static site. Adapters here turn a request into facts; classify.rs decides; store writes.
mod classify;
mod days;
mod page;
mod ranges;
mod types;

use std::net::IpAddr;
use std::sync::OnceLock;
use days::day_before;
use types::*;
use worker::wasm_bindgen::JsValue;
use worker::*;

// A referrer on the host this Worker serves (with or without www.) is an internal click; any other site,
// including our other sites, is a referral. One Worker serves one site, so no host list is needed.
pub(crate) fn own_host(url: &Url) -> String {
    let h = url.host_str().unwrap_or("").to_ascii_lowercase();
    h.strip_prefix("www.").unwrap_or(&h).to_string()
}

fn book() -> &'static ranges::Book {
    static BOOK: OnceLock<ranges::Book> = OnceLock::new();
    BOOK.get_or_init(ranges::Book::build)
}

pub(crate) fn today() -> String {
    day_before(0)
}

/// The D1 binding is DB here; the live site's config calls it VISITS, so either works.
pub(crate) fn db_of(env: &Env) -> Result<D1Database> {
    env.d1("DB").or_else(|_| env.d1("VISITS"))
}

pub(crate) fn s(v: &str) -> JsValue {
    JsValue::from_str(v)
}

// ---------- store ----------
async fn record(db: &D1Database, h: &Hit) -> Result<()> {
    db.prepare(
        "INSERT INTO hits (day, path, kind, ref, referrer_host, audience, operator, product, n) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1) \
         ON CONFLICT (day, path, kind, ref, referrer_host, audience, operator, product) DO UPDATE SET n = n + 1",
    )
    .bind(&[s(&h.day), s(&h.path), s(h.kind.name()), s(&h.ref_tag), s(&h.referrer_host), s(h.audience.name()), s(&h.operator), s(&h.product)])?
    .run()
    .await?;
    Ok(())
}

// ---------- adapters ----------
fn hit_of(req: &Request, url: &Url, kind: PageKind) -> Hit {
    let ua = req.headers().get("user-agent").ok().flatten();
    let ip: Option<IpAddr> = req.headers().get("cf-connecting-ip").ok().flatten().and_then(|v| v.parse().ok());
    let caller = classify::caller(book(), ua.as_deref(), ip);
    let (operator, product) = match &caller {
        Caller::Agent { operator, product, .. } => (operator.name().to_string(), product.name().to_string()),
        _ => (String::new(), String::new()),
    };
    // ?ref= is ours; utm_source= is how ChatGPT and most others tag a link (utm_source=chatgpt.com).
    let tag_of = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| classify::ref_tag(&v)).filter(|t| !t.is_empty());
    let ref_tag = tag_of("ref").or_else(|| tag_of("utm_source")).unwrap_or_default();
    let referrer_host = req.headers().get("referer").ok().flatten().map(|r| classify::referrer_host(&r, &[own_host(url).as_str()])).unwrap_or_default();
    Hit { day: today(), path: url.path().chars().take(200).collect(), kind, ref_tag, referrer_host, audience: classify::audience(&caller), operator, product }
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn read_api(req: &Request, url: &Url, env: &Env) -> Result<Response> {
    let secret = match env.secret("READ_TOKEN") {
        Ok(t) => t.to_string(),
        Err(_) => return Response::error("the read token is not set", 503),
    };
    let given = req.headers().get("authorization")?.unwrap_or_default();
    let given = given.strip_prefix("Bearer ").unwrap_or("");
    if secret.is_empty() || !same(given, &secret) {
        return Response::error("send the read token as: Authorization: Bearer <token>", 401);
    }
    let q = |k: &str| url.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string());
    let db = db_of(&env)?;
    match url.path() {
        "/api/v1/counts" => {
            let days: i64 = q("days").and_then(|v| v.parse().ok()).unwrap_or(7).clamp(1, 365);
            let from = day_before(days - 1);
            let edge = db
                .prepare("SELECT path, kind, ref, referrer_host, audience, operator, product, SUM(n) AS n FROM hits WHERE day >= ?1 GROUP BY path, kind, ref, referrer_host, audience, operator, product ORDER BY n DESC")
                .bind(&[s(&from)])?.all().await?.results::<serde_json::Value>()?;
            let beacon = db
                .prepare("SELECT path, ref, referrer_host, SUM(n) AS n FROM visits WHERE day >= ?1 GROUP BY path, ref, referrer_host ORDER BY n DESC")
                .bind(&[s(&from)])?.all().await?.results::<serde_json::Value>()?;
            // The same reports with the day kept, so a reader can say how many people came on a day.
            let beacon_by_day = db
                .prepare("SELECT day, path, ref, referrer_host, n FROM visits WHERE day >= ?1 ORDER BY day DESC, n DESC")
                .bind(&[s(&from)])?.all().await?.results::<serde_json::Value>()?;
            Response::from_json(&serde_json::json!({
                "from": from,
                "note": "edge: every counted request, by who asked (audience). beacon: pages whose JavaScript ran, so a person; a person is also in edge as unconfirmed-browser. beacon_by_day: the same reports, one row per day",
                "edge": edge, "beacon": beacon, "beacon_by_day": beacon_by_day
            }))
        }
        "/api/v1/hits" => {
            let limit: i64 = q("limit").and_then(|v| v.parse().ok()).unwrap_or(100).clamp(1, 1000);
            let rows = db
                .prepare("SELECT day, path, kind, ref, referrer_host, audience, operator, product, n FROM hits ORDER BY day DESC, n DESC LIMIT ?1")
                .bind(&[JsValue::from_f64(limit as f64)])?.all().await?.results::<serde_json::Value>()?;
            Response::from_json(&serde_json::json!({ "hits": rows }))
        }
        "/api/v1/views" => {
            let days: i64 = q("days").and_then(|v| v.parse().ok()).unwrap_or(7).clamp(1, 365);
            let from = day_before(days - 1);
            let views = db
                .prepare("SELECT * FROM views WHERE day >= ?1 ORDER BY at DESC LIMIT 5000")
                .bind(&[s(&from)])?.all().await?.results::<serde_json::Value>()?;
            Response::from_json(&serde_json::json!({ "from": from, "note": "one row per page view whose script ran; ip, visitor and place are empty for EU, EEA and UK visitors; visitor = sha256(salt, ip, user agent)", "views": views }))
        }
        "/api/v1/components" => {
            // per component: views that showed it, views that acted on it, and the counts, so seen 900 / clicked 9 reads off one row
            let days: i64 = q("days").and_then(|v| v.parse().ok()).unwrap_or(7).clamp(1, 365);
            let from = day_before(days - 1);
            let path = q("path").unwrap_or_default();
            let rows = db
                .prepare("SELECT path, component, area, section, kind, COUNT(DISTINCT view) AS views, SUM(n) AS n, SUM(ms) AS ms FROM component_events \
                          WHERE day >= ?1 AND (?2 = '' OR path = ?2) GROUP BY path, component, area, section, kind ORDER BY path, component, kind")
                .bind(&[s(&from), s(&path)])?.all().await?.results::<serde_json::Value>()?;
            Response::from_json(&serde_json::json!({ "from": from, "note": "views: page views with that event on that component; n: how many times; ms: total time in view (seen only)", "components": rows }))
        }
        _ => Response::error("not found: /api/v1/counts, /api/v1/hits, /api/v1/views or /api/v1/components (?days=7, &path=)", 404),
    }
}

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path().to_string();
    if matches!(path.as_str(), "/api/hit" | "/api/engaged" | "/api/events") {
        return page::page_event(req, &env, &path).await;
    }
    if path.starts_with("/api/v1/") {
        return read_api(&req, &url, &env).await;
    }
    let counted = if req.method() == Method::Get { classify::kind_of(&path) } else { None };
    let hit = counted.map(|kind| hit_of(&req, &url, kind));
    let resp = env.assets("ASSETS")?.fetch_request(req).await?;
    if let Some(hit) = hit {
        // only a served file counts: a 404 is not a visit
        if resp.status_code() == 200 {
            let db = db_of(&env)?;
            ctx.wait_until(async move {
                if let Err(e) = record(&db, &hit).await {
                    console_error!("kokumi: dropped a hit ({}): {e}", hit.path);
                }
            });
        }
    }
    Ok(resp)
}
