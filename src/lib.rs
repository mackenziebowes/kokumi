//! Kokumi slice 1: counts visits to pages, markdown twins and llms.txt at the edge, then serves the
//! static site. Adapters here turn a request into facts; classify.rs decides; store writes.
mod classify;
mod ranges;
mod types;

use std::net::IpAddr;
use std::sync::OnceLock;
use types::*;
use worker::wasm_bindgen::JsValue;
use worker::*;

// A referrer on the host this Worker serves (with or without www.) is an internal click; any other site,
// including our other sites, is a referral. One Worker serves one site, so no host list is needed.
fn own_host(url: &Url) -> String {
    let h = url.host_str().unwrap_or("").to_ascii_lowercase();
    h.strip_prefix("www.").unwrap_or(&h).to_string()
}

fn book() -> &'static ranges::Book {
    static BOOK: OnceLock<ranges::Book> = OnceLock::new();
    BOOK.get_or_init(ranges::Book::build)
}

fn today() -> String {
    day_before(0)
}

/// The D1 binding is DB here; the live site's config calls it VISITS, so either works.
fn db_of(env: &Env) -> Result<D1Database> {
    env.d1("DB").or_else(|_| env.d1("VISITS"))
}

fn s(v: &str) -> JsValue {
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

/// Pane's JS beacon, same contract as tools/deploy/visits.ts: one more for (day, path, ref, referrer host).
async fn beacon(mut req: Request, env: &Env) -> Result<Response> {
    if req.method() != Method::Post {
        let h = Headers::new();
        h.set("allow", "POST")?;
        return Ok(Response::empty()?.with_status(405).with_headers(h));
    }
    let body: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let path = match body["path"].as_str() {
        Some(p) if p.starts_with('/') => p.split(['?', '#']).next().unwrap_or("").chars().take(200).collect::<String>(),
        _ => return Ok(Response::empty()?.with_status(204)),
    };
    let ref_tag = classify::ref_tag(body["ref"].as_str().unwrap_or(""));
    let own = req.url().map(|u| own_host(&u)).unwrap_or_default();
    let referrer_host = classify::referrer_host(body["referrer"].as_str().unwrap_or(""), &[own.as_str()]);
    db_of(&env)?
        .prepare("INSERT INTO visits (day, path, ref, referrer_host, n) VALUES (?1,?2,?3,?4,1) ON CONFLICT (day, path, ref, referrer_host) DO UPDATE SET n = n + 1")
        .bind(&[s(&today()), s(&path), s(&ref_tag), s(&referrer_host)])?
        .run()
        .await?;
    Ok(Response::empty()?.with_status(204))
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
        _ => Response::error("not found: /api/v1/counts?days=7 or /api/v1/hits?limit=100", 404),
    }
}

fn day_before(n: i64) -> String {
    let ms = Date::now().as_millis() as i64 - n * 86_400_000;
    let days = ms / 86_400_000;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path().to_string();
    if path == "/api/hit" {
        return beacon(req, &env).await;
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
