fn main() {
    let input: String = std::io::read_to_string(std::io::stdin()).unwrap();
    let trimmed: String = input.trim().to_string();
    let count: f64 = trimmed.split("
").map(|s| s.to_string()).collect::<Vec<String>>().len() as f64;
    println!("{} {}", "count:", count);
    println!("{} {}", "content:", trimmed);
}