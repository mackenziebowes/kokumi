//! The domain, as approved in design act #1990 (delta: no fingerprints, no IP stored in slice 1).

/// What kind of file a counted request asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    Page,
    MarkdownTwin,
    LlmsTxt,
}
impl PageKind {
    pub fn name(self) -> &'static str {
        match self {
            PageKind::Page => "page",
            PageKind::MarkdownTwin => "twin",
            PageKind::LlmsTxt => "llms.txt",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    OpenAi,
    Google,
    Microsoft,
    Perplexity,
    Anthropic,
}
impl Operator {
    pub fn name(self) -> &'static str {
        match self {
            Operator::OpenAi => "OpenAI",
            Operator::Google => "Google",
            Operator::Microsoft => "Microsoft",
            Operator::Perplexity => "Perplexity",
            Operator::Anthropic => "Anthropic",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentProduct {
    GptBot,
    ChatGptUser,
    OaiSearchBot,
    Googlebot,
    GoogleUserTriggered,
    Bingbot,
    PerplexityBot,
    PerplexityUser,
    ClaudeBot,
    ClaudeUser,
    ClaudeSearchBot,
}
impl AgentProduct {
    pub fn name(self) -> &'static str {
        match self {
            AgentProduct::GptBot => "GPTBot",
            AgentProduct::ChatGptUser => "ChatGPT-User",
            AgentProduct::OaiSearchBot => "OAI-SearchBot",
            AgentProduct::Googlebot => "Googlebot",
            AgentProduct::GoogleUserTriggered => "Google-user-triggered",
            AgentProduct::Bingbot => "Bingbot",
            AgentProduct::PerplexityBot => "PerplexityBot",
            AgentProduct::PerplexityUser => "Perplexity-User",
            AgentProduct::ClaudeBot => "ClaudeBot",
            AgentProduct::ClaudeUser => "Claude-User",
            AgentProduct::ClaudeSearchBot => "Claude-SearchBot",
        }
    }
}

/// Which published file an IP was matched against (or would have been).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeSource(pub &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotVerified {
    OutsideOperatorRanges(RangeSource),
    OperatorPublishesNoRanges,
    NoClientIp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentIdentity {
    Verified(RangeSource),
    Claimed(NotVerified),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    Agent { operator: Operator, product: AgentProduct, identity: AgentIdentity },
    LooksLikeBrowser,
    Unrecognised,
    NoUserAgent,
}

/// What the read API calls each edge request. A person is counted by the page beacon (visits table), not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    VerifiedAgent,
    ClaimedAgent,
    UnconfirmedBrowser,
    Unrecognised,
}
impl Audience {
    pub fn name(self) -> &'static str {
        match self {
            Audience::VerifiedAgent => "verified-agent",
            Audience::ClaimedAgent => "claimed-agent",
            Audience::UnconfirmedBrowser => "unconfirmed-browser",
            Audience::Unrecognised => "unrecognised",
        }
    }
}

/// One counted request: what is stored (no IP, no user agent text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub day: String,
    pub path: String,
    pub kind: PageKind,
    pub ref_tag: String,
    pub referrer_host: String,
    pub audience: Audience,
    pub operator: String,
    pub product: String,
}

// ---------- slice 2: who viewed, how far they read, what they clicked (design #1508 rev 6) ----------

/// Visitors whose data protection law needs consent for this: EU, EEA and UK. They are counted, never keyed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visitor {
    /// The IP is kept, with a key for the device: sha256(salt, ip, user agent), so one IP with two browsers is two visitors.
    Keyed { ip: String, key: String, place: Place },
    Anonymous { country: String },
}

/// What Cloudflare says about the IP, at no cost: no third-party lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub country: String,
    pub region: String,
    pub city: String,
    pub asn: u32,
    pub network: String, // the AS organisation: an ISP, a company, or a cloud
}

/// What the page script sends. The view id is random per page load, held only in the page's memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageEvent {
    Viewed { view: String, path: String, ref_tag: String, referrer_host: String, screen: String, language: String },
    /// Sent each time the page is hidden; the totals so far, so the server keeps the largest.
    Engaged { view: String, visible_ms: u32, deepest_scroll: u8 },
    /// Component reports since the last batch: deltas, so the server adds them.
    Components { view: String, path: String, events: Vec<ComponentEvent> },
}

/// One component on one page view. `component` is named from the page itself (figure:<name>,
/// link:<path|#anchor|host>, toggle:<name>, section:<id>, button:<text>, form:<id>, code:<section>#n,
/// image:<file>), or from data-track="<name>" where the page sets one. `area` is where on the page it sits
/// (header, toc, article, article-foot, footer, page); `section` is the nearest heading above it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentEvent {
    pub component: String,
    pub area: String,
    pub section: String,
    pub interaction: Interaction,
    pub n: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// At least half of it was on screen for a second (a heading: on screen at all); `ms` is time in view.
    Seen { ms: u32 },
    Clicked,
    Opened,
    Closed,
    Played,
    Copied,
    Submitted,
}
impl Interaction {
    pub fn parse(kind: &str, ms: u32) -> Option<Interaction> {
        Some(match kind {
            "seen" => Interaction::Seen { ms },
            "clicked" => Interaction::Clicked,
            "opened" => Interaction::Opened,
            "closed" => Interaction::Closed,
            "played" => Interaction::Played,
            "copied" => Interaction::Copied,
            "submitted" => Interaction::Submitted,
            _ => return None,
        })
    }
    pub fn parts(self) -> (&'static str, u32) {
        match self {
            Interaction::Seen { ms } => ("seen", ms),
            Interaction::Clicked => ("clicked", 0),
            Interaction::Opened => ("opened", 0),
            Interaction::Closed => ("closed", 0),
            Interaction::Played => ("played", 0),
            Interaction::Copied => ("copied", 0),
            Interaction::Submitted => ("submitted", 0),
        }
    }
}
