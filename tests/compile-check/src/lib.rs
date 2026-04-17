fn main() {
    let f: Box<dyn Fn(f64) -> f64> = |x: f64| x + 1.0;
    println!("{}", f(5.0));
}
