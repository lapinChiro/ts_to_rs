struct Dog {
    kind: String,
    name: String,
    breed: String,
}

struct Cat {
    kind: String,
    name: String,
    indoor: bool,
}

fn describeDog(d: Dog) -> String {
    d.name + " is a " + &d.breed
}

fn describeCat(c: Cat) -> String {
    if c.indoor {
        return c.name + " is indoor";
    }
    c.name + " is outdoor"
}

fn main() {
    let mut d: Dog = Dog { kind: "dog".to_string(), name: "Rex".to_string(), breed: "Labrador".to_string() };
    let mut c: Cat = Cat { kind: "cat".to_string(), name: "Whiskers".to_string(), indoor: true };
    println!("{} {}", "dog:", describeDog(d));
    println!("{} {}", "cat:", describeCat(c));
}