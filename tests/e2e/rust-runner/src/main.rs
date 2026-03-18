fn main() {
    let mut count: f64 = 0.0;
    count = count + 1.0;
    println!("{} {}", "after ++:", count);
    count = count - 1.0;
    println!("{} {}", "after --:", count);
    let mut x: f64 = 5.0;
    x = x + 1.0;
    x = x + 1.0;
    x = x + 1.0;
    println!("{} {}", "x after 3 increments:", x);
    let mut n: f64 = 3.0;
    while n > 0.0 {
        println!("{} {}", "n:", n);
        n = n - 1.0;
    }
}