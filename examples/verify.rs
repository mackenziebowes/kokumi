//! Demonstration: how a request is classified. `cargo run --example verify`
#[path = "../src/classify.rs"] mod classify;
#[path = "../src/ranges.rs"] mod ranges;
#[path = "../src/types.rs"] mod types;

fn main() {
    let book = ranges::Book::build();
    let show = |label: &str, ua: &str, ip: &str| {
        let c = classify::caller(&book, Some(ua), ip.parse().ok());
        println!("{label:<34} {ip:<18} -> {:?} => {}", c, classify::audience(&c).name());
    };
    // 23.98.142.176 is inside OpenAI's published chatgpt-user list; 203.0.113.9 is a documentation address
    show("GPTBot from OpenAI's range", "GPTBot/1.2", "20.171.206.1");
    show("ChatGPT-User from its range", "Mozilla/5.0 ChatGPT-User/1.0", "23.98.142.176");
    show("GPTBot from elsewhere (spoof)", "GPTBot/1.2", "203.0.113.9");
    show("Claude-User from Anthropic outbound", "Claude-User", "160.79.105.7");
    show("ClaudeBot (no published list)", "ClaudeBot/1.0", "160.79.105.7");
    show("Perplexity-User, no IP header", "Perplexity-User/1.0", "");
    show("Chrome", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120 Safari/537.36", "203.0.113.9");
    show("curl", "curl/8.4.0", "203.0.113.9");
    for p in ["/", "/about", "/about.md", "/posts/a-post", "/llms.txt", "/images/a.png", "/lesson.js"] {
        println!("path {p:<14} -> {:?}", classify::kind_of(p));
    }
}
