fn main() {
    let name: String = "World".to_string();
    let n: f64 = 42.0;
    println!("{}", format!("Hello {}", name));
    println!("{}", format!("The answer is {}", n));
    println!("{}", format!("{} + {} = {}", n, n, n + n));
}