//! The page script's reports (/api/hit, /api/engaged, /api/click): a view keyed by IP, how long and how far it was read, what was clicked.
use crate::types::*;
use crate::{classify, db_of, own_host, s, today};
use worker::wasm_bindgen::JsValue;
use worker::*;

/// EU, EEA and UK visitors are counted without their IP or a device key (design #1508 rev 6, assumption 4).
fn visitor_of(req: &Request, salt: &str) -> Visitor {
    // read request.cf field by field: the crate's accessors unwrap and can abort the Worker
    let cf = worker::js_sys::Reflect::get(req.inner(), &s("cf")).unwrap_or(JsValue::UNDEFINED);
    let field = |k: &str| worker::js_sys::Reflect::get(&cf, &s(k)).ok().filter(|v| !v.is_undefined() && !v.is_null());
    let text_of = |k: &str| field(k).and_then(|v| v.as_string()).unwrap_or_default();
    let country = text_of("country");
    let protected = text_of("isEUCountry") == "1" || matches!(country.as_str(), "GB" | "IS" | "LI" | "NO" | "CH");
    let ip = req.headers().get("cf-connecting-ip").ok().flatten().unwrap_or_default();
    if protected || ip.is_empty() || salt.is_empty() {
        return Visitor::Anonymous { country };
    }
    use sha2::{Digest, Sha256};
    let ua = req.headers().get("user-agent").ok().flatten().unwrap_or_default();
    let digest = Sha256::digest(format!("{salt}\n{ip}\n{ua}").as_bytes());
    let key: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    let place = Place {
        country,
        region: text_of("region"),
        city: text_of("city"),
        asn: field("asn").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
        network: text_of("asOrganization"),
    };
    Visitor::Keyed { ip, key, place }
}

fn text(v: &serde_json::Value, k: &str, max: usize) -> String {
    v[k].as_str().unwrap_or("").chars().take(max).collect()
}

/// The page script's three reports. Viewed keeps Pane's visits count (same contract as visits.ts) and adds a view row.
pub async fn page_event(mut req: Request, env: &Env, route: &str) -> Result<Response> {
    if req.method() != Method::Post {
        let h = Headers::new();
        h.set("allow", "POST")?;
        return Ok(Response::empty()?.with_status(405).with_headers(h));
    }
    let salt = env.secret("VISITOR_SALT").map(|s| s.to_string()).unwrap_or_default();
    let visitor = visitor_of(&req, &salt);
    let body: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let own = req.url().map(|u| own_host(&u)).unwrap_or_default();
    let view = text(&body, "view", 40);
    let event = match route {
        "/api/hit" => {
            let path = match body["path"].as_str() {
                Some(p) if p.starts_with('/') => p.split(['?', '#']).next().unwrap_or("").chars().take(200).collect::<String>(),
                _ => return Ok(Response::empty()?.with_status(204)),
            };
            PageEvent::Viewed {
                view,
                path,
                ref_tag: classify::ref_tag(body["ref"].as_str().unwrap_or("")),
                referrer_host: classify::referrer_host(body["referrer"].as_str().unwrap_or(""), &[own.as_str()]),
                screen: text(&body, "screen", 10),
                language: text(&body, "language", 5),
            }
        }
        "/api/engaged" => PageEvent::Engaged {
            view,
            visible_ms: body["visible_ms"].as_u64().unwrap_or(0).min(86_400_000) as u32,
            deepest_scroll: body["deepest_scroll"].as_u64().unwrap_or(0).min(100) as u8,
        },
        _ => {
            let t = &body["target"];
            let target = match t["kind"].as_str() {
                Some("email") => Target::Email,
                Some("outbound") => Target::Outbound { host: text(t, "host", 100) },
                Some("internal") => Target::Internal { path: text(t, "path", 200) },
                _ => return Ok(Response::empty()?.with_status(204)),
            };
            PageEvent::Clicked { view, target }
        }
    };
    let db = db_of(env)?;
    store_page_event(&db, &event, &visitor).await?;
    Ok(Response::empty()?.with_status(204))
}

async fn store_page_event(db: &D1Database, e: &PageEvent, v: &Visitor) -> Result<()> {
    let at = JsValue::from_f64(Date::now().as_millis() as f64);
    match e {
        PageEvent::Viewed { view, path, ref_tag, referrer_host, screen, language } => {
            db.prepare("INSERT INTO visits (day, path, ref, referrer_host, n) VALUES (?1,?2,?3,?4,1) ON CONFLICT (day, path, ref, referrer_host) DO UPDATE SET n = n + 1")
                .bind(&[s(&today()), s(path), s(ref_tag), s(referrer_host)])?.run().await?;
            if view.is_empty() { return Ok(()); } // the old script sends no view id
            let null = JsValue::NULL;
            let (ip, key, country, region, city, asn, network) = match v {
                Visitor::Keyed { ip, key, place } => (s(ip), s(key), s(&place.country), s(&place.region), s(&place.city), JsValue::from_f64(place.asn as f64), s(&place.network)),
                Visitor::Anonymous { country } => (null.clone(), null.clone(), s(country), null.clone(), null.clone(), null.clone(), null),
            };
            db.prepare("INSERT OR IGNORE INTO views (view, day, at, path, ref, referrer_host, screen, language, ip, visitor, country, region, city, asn, network) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)")
                .bind(&[s(view), s(&today()), at, s(path), s(ref_tag), s(referrer_host), s(screen), s(language), ip, key, country, region, city, asn, network])?.run().await?;
        }
        PageEvent::Engaged { view, visible_ms, deepest_scroll } => {
            db.prepare("UPDATE views SET visible_ms = MAX(visible_ms, ?2), deepest_scroll = MAX(deepest_scroll, ?3) WHERE view = ?1")
                .bind(&[s(view), JsValue::from_f64(*visible_ms as f64), JsValue::from_f64(*deepest_scroll as f64)])?.run().await?;
        }
        PageEvent::Clicked { view, target } => {
            let (kind, value) = target.parts();
            db.prepare("INSERT INTO clicks (view, at, kind, target) VALUES (?1,?2,?3,?4)")
                .bind(&[s(view), at, s(kind), s(value)])?.run().await?;
        }
    }
    Ok(())
}

