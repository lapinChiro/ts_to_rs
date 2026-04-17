use std::collections::HashMap;
fn getOrSet(cache: HashMap<String, String>, key: String) -> String {
    let mut cache = cache;
    cache
        .entry(key.clone())
        .or_insert_with(|| "default:".to_string() + &key)
        .clone()
}

fn main() {
    println!("ok");
}
