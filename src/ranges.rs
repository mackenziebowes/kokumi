//! Operators' published IP lists, vendored in ranges/ (see ranges/SOURCES.md).
use crate::types::RangeSource;
use serde::Deserialize;
use std::net::IpAddr;

#[derive(Clone, Copy)]
pub struct Cidr {
    v6: bool,
    net: u128,
    bits: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Cidr> {
        let (addr, bits) = s.split_once('/')?;
        let bits: u8 = bits.parse().ok()?;
        match addr.parse::<IpAddr>().ok()? {
            IpAddr::V4(a) if bits <= 32 => Some(Cidr { v6: false, net: u32::from(a) as u128, bits }),
            IpAddr::V6(a) if bits <= 128 => Some(Cidr { v6: true, net: u128::from(a), bits }),
            _ => None,
        }
    }
    pub fn contains(&self, ip: IpAddr) -> bool {
        let (v6, value, width) = match ip {
            IpAddr::V4(a) => (false, u32::from(a) as u128, 32u32),
            IpAddr::V6(a) => (true, u128::from(a), 128u32),
        };
        if v6 != self.v6 {
            return false;
        }
        let shift = width - self.bits as u32;
        (value >> shift) == (self.net >> shift)
    }
}

#[derive(Deserialize)]
struct File {
    prefixes: Vec<Prefix>,
}
#[derive(Deserialize)]
struct Prefix {
    #[serde(rename = "ipv4Prefix")]
    v4: Option<String>,
    #[serde(rename = "ipv6Prefix")]
    v6: Option<String>,
}

pub struct RangeList {
    pub source: RangeSource,
    cidrs: Vec<Cidr>,
}
impl RangeList {
    fn from_json(source: &'static str, json: &str) -> RangeList {
        let file: File = serde_json::from_str(json).expect("vendored range file parses");
        let cidrs = file
            .prefixes
            .iter()
            .filter_map(|p| p.v4.as_deref().or(p.v6.as_deref()).and_then(Cidr::parse))
            .collect();
        RangeList { source: RangeSource(source), cidrs }
    }
    fn from_cidrs(source: &'static str, list: &[&str]) -> RangeList {
        RangeList { source: RangeSource(source), cidrs: list.iter().filter_map(|s| Cidr::parse(s)).collect() }
    }
    pub fn contains(&self, ip: IpAddr) -> bool {
        self.cidrs.iter().any(|c| c.contains(ip))
    }
}

pub struct Book {
    pub gptbot: RangeList,
    pub chatgpt_user: RangeList,
    pub oai_searchbot: RangeList,
    pub googlebot: RangeList,
    pub google_user_triggered: Vec<RangeList>,
    pub bingbot: RangeList,
    pub perplexitybot: RangeList,
    pub perplexity_user: RangeList,
    pub anthropic_outbound: RangeList,
}

impl Book {
    /// Parsed once per isolate (the Worker's CPU budget is small).
    pub fn build() -> Book {
        Book {
            gptbot: RangeList::from_json("openai.com/gptbot.json", include_str!("../ranges/gptbot.json")),
            chatgpt_user: RangeList::from_json("openai.com/chatgpt-user.json", include_str!("../ranges/chatgpt-user.json")),
            oai_searchbot: RangeList::from_json("openai.com/searchbot.json", include_str!("../ranges/searchbot.json")),
            googlebot: RangeList::from_json("developers.google.com/.../googlebot.json", include_str!("../ranges/googlebot.json")),
            google_user_triggered: vec![
                RangeList::from_json("developers.google.com/.../user-triggered-fetchers.json", include_str!("../ranges/user-triggered-fetchers.json")),
                RangeList::from_json("developers.google.com/.../user-triggered-fetchers-google.json", include_str!("../ranges/user-triggered-fetchers-google.json")),
                RangeList::from_json("developers.google.com/.../special-crawlers.json", include_str!("../ranges/special-crawlers.json")),
            ],
            bingbot: RangeList::from_json("bing.com/toolbox/bingbot.json", include_str!("../ranges/bingbot.json")),
            perplexitybot: RangeList::from_json("perplexity.com/perplexitybot.json", include_str!("../ranges/perplexitybot.json")),
            perplexity_user: RangeList::from_json("perplexity.com/perplexity-user.json", include_str!("../ranges/perplexity-user.json")),
            // docs.anthropic.com/en/api/ip-addresses, "Outbound IP addresses" (an HTML page, copied by hand 2026-10-17)
            anthropic_outbound: RangeList::from_cidrs("docs.anthropic.com/en/api/ip-addresses (outbound)", &["160.79.104.0/21"]),
        }
    }
}
