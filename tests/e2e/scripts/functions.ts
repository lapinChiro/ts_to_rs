// 再帰
function factorial(n: number): number {
    if (n <= 1) {
        return 1;
    }
    return n * factorial(n - 1);
}

// デフォルト引数
function greet(name: string, greeting: string = "Hello"): string {
    return greeting + " " + name;
}

function main(): void {
    console.log("factorial 5:", factorial(5));
    console.log("factorial 1:", factorial(1));
    console.log("factorial 10:", factorial(10));
    console.log(greet("World"));
    console.log(greet("World", "Hi"));
}
