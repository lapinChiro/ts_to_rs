fn getLength(s: String) -> f64 {
    s.len() as f64
}

fn checkPrefix(s: String) -> bool {
    s.starts_with("hello".to_string())
}

fn normalize(s: String) -> String {
    s.trim().to_string().to_lowercase()
}

fn replaceChar(s: String) -> String {
    s.replacen("a", "b".to_string(), 1)
}

fn hasContent(s: String) -> bool {
    s.contains(&"x".to_string()) && !s.ends_with("z".to_string())
}

fn shout(s: String) -> String {
    s.to_uppercase()
}
