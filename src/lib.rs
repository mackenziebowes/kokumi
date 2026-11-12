//! Kokumi: counts visits to pages, markdown twins and llms.txt at the edge, then serves the
//! static site. Adapters here turn a request into facts; classify.rs decides; store writes.
mod classify;
mod days;
mod page;
mod ranges;
mod read;
mod store;
mod tool;
mod types;

use std::net::IpAddr;
use std::sync::OnceLock;
use days::day_before;
use read::read_api;
use store::record;
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
    let referrer_path = match kind {
        PageKind::Page => req.headers().get("referer").ok().flatten().map(|r| classify::referrer_path(&r, &[own_host(url).as_str()])).unwrap_or_default(),
        _ => String::new(),
    };
    Hit { day: today(), path: url.path().chars().take(200).collect(), kind, ref_tag, referrer_host, referrer_path, audience: classify::audience(&caller), operator, product }
}

/// /projects and /projects/* go to the PROJECTS Worker (#3086), which fills the page from the feed and answers the
/// static page itself when the feed is slow. Only if the binding is missing or errors does this serve the static asset.
fn is_projects(path: &str) -> bool {
    path == "/projects" || path.starts_with("/projects/")
}

async fn serve(req: Request, env: &Env, path: &str) -> Result<Response> {
    if is_projects(path) && req.method() == Method::Get {
        if let Ok(svc) = env.service("PROJECTS") {
            match req.clone().and_then(|r| Ok(r)) {
                Ok(copy) => match svc.fetch_request(copy).await {
                    Ok(resp) => return Ok(resp),
                    Err(e) => console_error!("kokumi: PROJECTS failed on {path}, serving the static page: {e}"),
                },
                Err(e) => console_error!("kokumi: could not copy the request for PROJECTS: {e}"),
            }
        }
    }
    env.assets("ASSETS")?.fetch_request(req).await
}

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path().to_string();
    if matches!(path.as_str(), "/api/hit" | "/api/engaged" | "/api/events") {
        return page::page_event(req, &env, &path).await;
    }
    if let Some(tool) = tool::route_of(&path) {
        return tool::forward(req, &env, &ctx, tool).await;
    }
    if path.starts_with("/api/v1/") {
        return read_api(&req, &url, &env).await;
    }
    let counted = if req.method() == Method::Get { classify::kind_of(&path) } else { None };
    let hit = counted.map(|kind| hit_of(&req, &url, kind));
    let resp = serve(req, &env, &path).await?;
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
