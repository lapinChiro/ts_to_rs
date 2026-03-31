#[derive(Debug, Clone, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

fn makePoint(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[derive(Debug, Clone, PartialEq)]
struct Info {
    name: String,
    value: f64,
}

fn createInfo(n: String, v: f64) -> Info {
    Info { name: n, value: v }
}

fn main() {
    let p: Point = makePoint(3.0, 4.0);
    println!("{} {}", "point x:", p.x);
    println!("{} {}", "point y:", p.y);
    let info: Info = createInfo("test".to_string(), 42.0);
    println!("{} {}", "info name:", info.name);
    println!("{} {}", "info value:", info.value);
}