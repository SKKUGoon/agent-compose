use serde_json::{json, Value};

pub fn workflow_input_from_text(text: &str) -> Value {
    let cleaned = text.trim();
    let mut title = cleaned
        .split('\n')
        .next()
        .unwrap_or("Untitled")
        .trim()
        .to_string();
    if let Some((head, _)) = title.split_once('.') {
        title = head.trim().to_string();
    }
    if title.is_empty() {
        title = "Untitled".to_string();
    }
    json!({
        "title": title,
        "summary": cleaned,
        "full_article": cleaned,
        "source": "cli",
        "link": "",
        "published_at": ""
    })
}
