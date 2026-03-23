#[derive(Debug, Clone, PartialEq)]
struct Greeter {
    name: String,
}

impl Greeter {
    fn new(name: String) -> Self {
        Self { name }
    }

    fn greet(&self, msg: String) -> String {
        format!("{}: {}", self.name, msg)
    }
}

fn main() {
    let g = Greeter::new("Alice".to_string());
    println!("{}", g.greet("hello"));
    let mut h = Greeter::new("Bob".to_string());
    println!("{}", h.greet("world"));
}
