fn increment(x: f64, step: Option<f64>) -> f64 {
    let step = step.unwrap_or(1.0);
    x + step
}

fn greet(name: Option<String>) -> String {
    let name = name.unwrap_or("world".to_string());
    "hello ".to_string() + &name
}

fn showFlag(v: Option<bool>) -> String {
    let v = v.unwrap_or(true);
    if v {
        return "on".to_string();
    }
    "off".to_string()
}

fn range(start: Option<f64>, end: Option<f64>, step: Option<f64>) -> f64 {
    let start = start.unwrap_or(0.0);
    let end = end.unwrap_or(10.0);
    let step = step.unwrap_or(1.0);
    let mut count: f64 = 0.0;
    let mut i: f64 = start;
    while i < end {
        count = count + 1.0;
        i = i + step;
    }
    count
}

pub fn init() {
    println!("{} {}", "inc 5:", increment(5.0, None));
    println!("{} {}", "inc 5 3:", increment(5.0, Some(3.0)));
    println!("{}", greet(Some("Alice".to_string())));
    println!("{}", greet(None));
    println!("{} {}", "flag:", showFlag(None));
    println!("{} {}", "flag false:", showFlag(Some(false)));
    println!("{} {}", "config:", createConfig(None));
    println!(
        "{} {}",
        "config true:",
        createConfig(Some(_TypeLit0 { verbose: true }))
    );
    println!("{} {}", "range default:", range(None, None, None));
    println!("{} {}", "range 2:", range(Some(2.0), None, None));
    println!("{} {}", "range 2 5:", range(Some(2.0), Some(5.0), None));
    println!(
        "{} {}",
        "range 0 10 2:",
        range(Some(0.0), Some(10.0), Some(2.0))
    );
}
