//! Pure: request facts in, domain values out. No I/O, no time, no storage.
use crate::ranges::{Book, RangeList};
use crate::types::*;
use std::net::IpAddr;

/// Which published list proves this product, or none exists.
enum Lists<'a> {
    One(&'a RangeList),
    Many(&'a [RangeList]),
    NonePublished,
}

fn agent_of(ua: &str) -> Option<(Operator, AgentProduct)> {
    let u = ua.to_ascii_lowercase();
    let table: [(&str, Operator, AgentProduct); 12] = [
        ("oai-searchbot", Operator::OpenAi, AgentProduct::OaiSearchBot),
        ("chatgpt-user", Operator::OpenAi, AgentProduct::ChatGptUser),
        ("gptbot", Operator::OpenAi, AgentProduct::GptBot),
        ("claude-searchbot", Operator::Anthropic, AgentProduct::ClaudeSearchBot),
        ("claude-user", Operator::Anthropic, AgentProduct::ClaudeUser),
        ("claudebot", Operator::Anthropic, AgentProduct::ClaudeBot),
        ("perplexity-user", Operator::Perplexity, AgentProduct::PerplexityUser),
        ("perplexitybot", Operator::Perplexity, AgentProduct::PerplexityBot),
        ("bingbot", Operator::Microsoft, AgentProduct::Bingbot),
        ("googlebot", Operator::Google, AgentProduct::Googlebot),
        ("google-read-aloud", Operator::Google, AgentProduct::GoogleUserTriggered),
        ("feedfetcher-google", Operator::Google, AgentProduct::GoogleUserTriggered),
    ];
    table.iter().find(|(needle, _, _)| u.contains(needle)).map(|(_, o, p)| (*o, *p))
}

fn lists_for<'a>(book: &'a Book, product: AgentProduct) -> Lists<'a> {
    match product {
        AgentProduct::GptBot => Lists::One(&book.gptbot),
        AgentProduct::ChatGptUser => Lists::One(&book.chatgpt_user),
        AgentProduct::OaiSearchBot => Lists::One(&book.oai_searchbot),
        AgentProduct::Googlebot => Lists::One(&book.googlebot),
        AgentProduct::GoogleUserTriggered => Lists::Many(&book.google_user_triggered),
        AgentProduct::Bingbot => Lists::One(&book.bingbot),
        AgentProduct::PerplexityBot => Lists::One(&book.perplexitybot),
        AgentProduct::PerplexityUser => Lists::One(&book.perplexity_user),
        AgentProduct::ClaudeUser | AgentProduct::ClaudeSearchBot => Lists::One(&book.anthropic_outbound),
        AgentProduct::ClaudeBot => Lists::NonePublished,
    }
}

fn identity(book: &Book, product: AgentProduct, ip: Option<IpAddr>) -> AgentIdentity {
    let lists = lists_for(book, product);
    let ip = match (ip, &lists) {
        (_, Lists::NonePublished) => return AgentIdentity::Claimed(NotVerified::OperatorPublishesNoRanges),
        (None, _) => return AgentIdentity::Claimed(NotVerified::NoClientIp),
        (Some(ip), _) => ip,
    };
    match lists {
        Lists::One(l) if l.contains(ip) => AgentIdentity::Verified(l.source),
        Lists::One(l) => AgentIdentity::Claimed(NotVerified::OutsideOperatorRanges(l.source)),
        Lists::Many(ls) => match ls.iter().find(|l| l.contains(ip)) {
            Some(l) => AgentIdentity::Verified(l.source),
            None => AgentIdentity::Claimed(NotVerified::OutsideOperatorRanges(ls[0].source)),
        },
        Lists::NonePublished => unreachable!(),
    }
}

pub fn caller(book: &Book, user_agent: Option<&str>, ip: Option<IpAddr>) -> Caller {
    let ua = match user_agent.map(str::trim).filter(|s| !s.is_empty()) {
        None => return Caller::NoUserAgent,
        Some(ua) => ua,
    };
    if let Some((operator, product)) = agent_of(ua) {
        return Caller::Agent { operator, product, identity: identity(book, product, ip) };
    }
    if ua.starts_with("Mozilla/") && (ua.contains("AppleWebKit") || ua.contains("Gecko/") || ua.contains("Firefox/")) {
        Caller::LooksLikeBrowser
    } else {
        Caller::Unrecognised
    }
}

pub fn audience(c: &Caller) -> Audience {
    match c {
        Caller::Agent { identity: AgentIdentity::Verified(_), .. } => Audience::VerifiedAgent,
        Caller::Agent { identity: AgentIdentity::Claimed(_), .. } => Audience::ClaimedAgent,
        Caller::LooksLikeBrowser => Audience::UnconfirmedBrowser,
        Caller::Unrecognised | Caller::NoUserAgent => Audience::Unrecognised,
    }
}

/// A counted request is a page, a markdown twin or llms.txt; anything else (images, fonts, scripts) is not counted.
pub fn kind_of(path: &str) -> Option<PageKind> {
    if path == "/llms.txt" || path == "/llms-full.txt" {
        return Some(PageKind::LlmsTxt);
    }
    let last = path.rsplit('/').next().unwrap_or("");
    if last.ends_with(".md") {
        return Some(PageKind::MarkdownTwin);
    }
    if last.is_empty() || last.ends_with(".html") || !last.contains('.') {
        return Some(PageKind::Page);
    }
    None
}

pub fn ref_tag(raw: &str) -> String {
    // dots become dashes, so utm_source=chatgpt.com is kept as "chatgpt-com"
    let r = raw.to_ascii_lowercase().replace('.', "-");
    let ok = !r.is_empty() && r.len() <= 40 && r.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok { r } else { String::new() }
}

/// Host of the Referer header, without "www.", and empty for our own site or anything unreadable.
pub fn referrer_host(referer: &str, own_hosts: &[&str]) -> String {
    let rest = referer.split_once("://").map(|(_, r)| r).unwrap_or("");
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").split(':').next().unwrap_or("").to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let valid = !host.is_empty() && host.len() <= 100 && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if valid && !own_hosts.contains(&host.as_str()) { host } else { String::new() }
}
