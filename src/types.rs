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
