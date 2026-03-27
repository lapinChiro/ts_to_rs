function increment(x: number, step: number = 1): number {
    return x + step;
}

function greet(name: string = "world"): string {
    return "hello " + name;
}

function showFlag(v: boolean = true): string {
    if (v) {
        return "on";
    }
    return "off";
}

function range(start: number = 0, end: number = 10, step: number = 1): number {
    let count: number = 0;
    let i: number = start;
    while (i < end) {
        count = count + 1;
        i = i + step;
    }
    return count;
}

function main(): void {
    console.log("inc 5:", increment(5));
    console.log("inc 5 3:", increment(5, 3));
    console.log(greet("Alice"));
    console.log(greet());
    console.log("flag:", showFlag());
    console.log("flag false:", showFlag(false));
    console.log("range default:", range());
    console.log("range 2:", range(2));
    console.log("range 2 5:", range(2, 5));
    console.log("range 0 10 2:", range(0, 10, 2));
}
