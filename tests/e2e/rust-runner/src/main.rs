fn main() {
    let s: String = "Hello, World!".to_string();
    println!("{} {}", "upper:", s.to_uppercase());
    println!("{} {}", "lower:", s.to_lowercase());
    println!("{} {}", "includes:", s.contains("World"));
    println!("{} {}", "starts:", s.starts_with("Hello"));
    println!("{} {}", "trim:", "  spaces  ".trim().to_string());
    println!("{} {}", "split:", "a,b,c".split(",").collect::<Vec<&str>>().join(" "));
}