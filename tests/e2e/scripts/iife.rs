fn main() {
    let x: f64 = {
        let __iife = |n: f64| -> f64 { n * 2.0 };
        __iife(21.0)
    };
    println!("{} {}", "double:", x);
    let sum: f64 = {
        let __iife = |a: f64, b: f64| -> f64 { a + b };
        __iife(10.0, 20.0)
    };
    println!("{} {}", "sum:", sum);
    let msg: String = {
        let __iife = || -> String { "hello iife" };
        __iife()
    };
    println!("{} {}", "msg:", msg);
}
