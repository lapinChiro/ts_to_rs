fn factorial(n: f64) -> f64 {
    if n <= 1.0 {
        return 1.0;
    }
    n * factorial(n - 1.0)
}

fn greet(name: String, greeting: Option<String>) -> String {
    let greeting = greeting.unwrap_or("Hello".to_string());
    greeting + " " + &name
}

fn main() {
    println!("{} {}", "factorial 5:", factorial(5.0));
    println!("{} {}", "factorial 1:", factorial(1.0));
    println!("{} {}", "factorial 10:", factorial(10.0));
    println!("{}", greet("World".to_string(), None));
    println!("{}", greet("World".to_string(), Some("Hi".to_string())));
}
