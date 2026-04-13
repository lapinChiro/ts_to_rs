function identity<T>(value: T): T {
    return value;
}

function wrapValue<T, U>(value: T, label: U): T {
    return value;
}

// Multi-arg same type param: inference feedback from 1st arg to 2nd
function pair<T>(a: T, b: T): T {
    return a;
}

// Partial inference: T inferred, U inferred independently
function triple<T, U>(x: T, y: U, z: T): T {
    return z;
}

// Monomorphization: T extends number → T is replaced with f64
class NumberBox<T extends number> {
    value: T;
    constructor(val: T) {
        this.value = val;
    }
    double(): number {
        return this.value * 2;
    }
}

function main(): void {
    console.log("number:", identity(42));
    console.log("bool:", identity(true));
    console.log("wrap num:", wrapValue(99, "label"));
    console.log("wrap bool:", wrapValue(true, 0));

    const box1 = new NumberBox(21);
    console.log("double:", box1.double());

    // Multi-arg same type param (I-001 inference feedback)
    console.log("pair num:", pair(10, 20));
    console.log("pair str:", pair("hello", "world"));
    console.log("triple:", triple("a", 1, "b"));
}
