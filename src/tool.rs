//! Tools behind Kokumi (#2117): a request to a tool's route is rate-limited, passed to the tool's Worker over a service
//! binding, answered, and recorded as one tool_events row from what the tool reports in `x-tool-outcome`.
//! The answer returns even if the record fails; the header is removed before the visitor sees the response.
use crate::types::*;
use crate::{classify, db_of, page, s, today};
use serde::Deserialize;
use std::net::IpAddr;
use worker::*;

/// One route, the binding that serves it and the name its rows carry. A new tool is a new entry and a binding.
struct Tool {
    route: &'static str,
    binding: &'static str,
    name: &'static str,
}
const TOOLS: &[Tool] = &[Tool { route: "/api/php-check", binding: "PHP_CHECK", name: "php-check" }];

/// What a tool reports about one call. Values are the tool's own words, kept short.
#[derive(Deserialize, Default)]
struct Reported {
    #[serde(default)]
    input: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    verdict: String,
    #[serde(default)]
    turnstile: String,
}

pub fn route_of(path: &str) -> Option<usize> {
    TOOLS.iter().position(|t| t.route == path)
}

fn short(v: &str, max: usize) -> String {
    v.chars().take(max).collect()
}

fn json(status: u16, message: &str) -> Result<Response> {
    Ok(Response::from_json(&serde_json::json!({ "error": message }))?.with_status(status))
}

pub async fn forward(req: Request, env: &Env, ctx: &Context, tool: usize) -> Result<Response> {
    let tool = &TOOLS[tool];
    if req.method() != Method::Post {
        let h = Headers::new();
        h.set("allow", "POST")?;
        return Ok(Response::empty()?.with_status(405).with_headers(h));
    }
    let salt = env.secret("VISITOR_SALT").map(|s| s.to_string()).unwrap_or_default();
    let visitor = page::visitor_of(&req, &salt);
    let ua = req.headers().get("user-agent").ok().flatten();
    let ip_text = req.headers().get("cf-connecting-ip").ok().flatten();
    let caller = classify::caller(crate::book(), ua.as_deref(), ip_text.as_deref().and_then(|v| v.parse::<IpAddr>().ok()));
    let (operator, product) = match &caller {
        Caller::Agent { operator, product, .. } => (operator.name().to_string(), product.name().to_string()),
        _ => (String::new(), String::new()),
    };
    let audience = classify::audience(&caller);
    let event = |r: Reported| ToolEvent { day: today(), at: Date::now().as_millis(), tool: tool.name.to_string(), reported: r, audience, operator: operator.clone(), product: product.clone(), visitor: visitor.clone() };

    let limited = match env.rate_limiter("TOOL_LIMIT") {
        Ok(limiter) => !limiter.limit(ip_text.clone().unwrap_or_else(|| "none".into())).await.map(|o| o.success).unwrap_or(true),
        Err(_) => false,
    };
    if limited {
        record(env, ctx, event(Reported { outcome: "rate_limited".into(), verdict: "none".into(), turnstile: "absent".into(), ..Default::default() }));
        return json(429, "Too many checks from this address. Wait a minute and try again.");
    }

    let Ok(service) = env.service(tool.binding) else {
        record(env, ctx, event(Reported { outcome: "worker_missing".into(), verdict: "none".into(), turnstile: "absent".into(), ..Default::default() }));
        return json(503, "The check is unavailable. Try again later.");
    };
    let mut res = match service.fetch_request(req).await {
        Ok(r) => r,
        Err(e) => {
            console_error!("kokumi: {} failed: {e}", tool.name);
            record(env, ctx, event(Reported { outcome: "worker_error".into(), verdict: "none".into(), turnstile: "absent".into(), ..Default::default() }));
            return json(502, "The check is unavailable. Try again later.");
        }
    };
    let reported: Reported = res.headers().get("x-tool-outcome").ok().flatten().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_else(|| Reported { outcome: "no_report".into(), ..Default::default() });
    record(env, ctx, event(reported));

    // the same answer without the internal header
    let status = res.status_code();
    let kept = ["content-type", "cache-control"];
    let headers = Headers::new();
    for k in kept {
        if let Some(v) = res.headers().get(k).ok().flatten() {
            headers.set(k, &v)?;
        }
    }
    Ok(Response::from_bytes(res.bytes().await?)?.with_status(status).with_headers(headers))
}

/// Writes the row after the response is on its way. A failed write is logged and the call is lost from the count.
fn record(env: &Env, ctx: &Context, e: ToolEvent) {
    let Ok(db) = db_of(env) else { return };
    ctx.wait_until(async move {
        let (visitor, country) = match &e.visitor {
            Visitor::Keyed { key, place, .. } => (s(key), place.country.clone()),
            Visitor::Anonymous { country } => (worker::wasm_bindgen::JsValue::NULL, country.clone()),
        };
        let r = e.reported;
        let run = db
            .prepare("INSERT INTO tool_events (day, at, tool, input, outcome, verdict, turnstile, audience, operator, product, visitor, country) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)")
            .bind(&[s(&e.day), (e.at as f64).into(), s(&e.tool), s(&short(&r.input, 253)), s(&short(&r.outcome, 40)), s(&short(&r.verdict, 40)), s(&short(&r.turnstile, 20)), s(e.audience.name()), s(&e.operator), s(&e.product), visitor, s(&short(&country, 8))]);
        match run {
            Ok(q) => {
                if let Err(err) = q.run().await {
                    console_error!("kokumi: dropped a tool row ({}): {err}", e.tool);
                }
            }
            Err(err) => console_error!("kokumi: bad tool row ({}): {err}", e.tool),
        }
    });
}

/// A call as recorded.
pub struct ToolEvent {
    pub day: String,
    pub at: u64,
    pub tool: String,
    reported: Reported,
    pub audience: Audience,
    pub operator: String,
    pub product: String,
    pub visitor: Visitor,
}
