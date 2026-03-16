fn main() {
    let mut nums: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let doubled = nums
        .iter()
        .map(|x: f64| -> f64 { x * 2.0 })
        .collect::<Vec<_>>();
    println!("{} {}", "map:", doubled);
    let evens = nums
        .iter()
        .filter(|x: f64| -> bool { x % 2.0 == 0.0 })
        .collect::<Vec<_>>();
    println!("{} {}", "filter:", evens);
    let found = nums.iter().find(|x: f64| -> bool { x > 3.0 });
    println!("{} {}", "find:", found);
    println!(
        "{} {}",
        "some >3:",
        nums.iter().any(|x: f64| -> bool { x > 3.0 })
    );
    println!(
        "{} {}",
        "some >10:",
        nums.iter().any(|x: f64| -> bool { x > 10.0 })
    );
    println!(
        "{} {}",
        "every >0:",
        nums.iter().all(|x: f64| -> bool { x > 0.0 })
    );
    println!(
        "{} {}",
        "every >3:",
        nums.iter().all(|x: f64| -> bool { x > 3.0 })
    );
    let sum = nums.iter().fold(0.0, |acc, x| -> f64 { acc + x });
    println!("{} {}", "reduce sum:", sum);
    let mut total: f64 = 0.0;
    nums.iter().for_each(|x: f64| -> () {
        total = total + x;
    });
    println!("{} {}", "forEach total:", total);
    println!(
        "{} {}",
        "indexOf 3:",
        nums.iter()
            .position(|item| *item == 3.0)
            .map(|i| i as f64)
            .unwrap_or(-1.0)
    );
    println!(
        "{} {}",
        "indexOf 99:",
        nums.iter()
            .position(|item| *item == 99.0)
            .map(|i| i as f64)
            .unwrap_or(-1.0)
    );
    let sliced = nums[1..3].to_vec();
    println!("{} {}", "slice 1,3:", sliced);
    let mut unsorted: Vec<f64> = vec![3.0, 1.0, 4.0, 1.0, 5.0];
    unsorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{} {}", "sort:", unsorted);
}
