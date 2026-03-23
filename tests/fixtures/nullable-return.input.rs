fn findTernary(x: bool) -> Option<String> {
    if x {
        Some("found".to_string())
    } else {
        None
    }
}

fn findDirect(x: bool) -> Option<String> {
    if x {
        return Some("found".to_string());
    }
    None
}
