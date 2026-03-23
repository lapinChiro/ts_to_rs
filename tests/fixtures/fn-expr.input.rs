fn applyTwice(x: f64) -> f64 {
    let double = |n: f64| -> f64 { n * 2.0 };
    double(double(x))
}

fn makeMessage(prefix: String) -> String {
    let format = |text: String| -> String { prefix + ": " + &text };
    format("hello".to_string())
}
