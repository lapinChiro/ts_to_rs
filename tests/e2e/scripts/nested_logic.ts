function fibonacci(n: number): number {
    if (n <= 1) {
        return n;
    }
    let a: number = 0;
    let b: number = 1;
    let i: number = 2;
    while (i <= n) {
        const temp: number = a + b;
        a = b;
        b = temp;
        i = i + 1;
    }
    return b;
}

function isPrime(n: number): boolean {
    if (n < 2) {
        return false;
    }
    let i: number = 2;
    while (i * i <= n) {
        if (n % i === 0) {
            return false;
        }
        i = i + 1;
    }
    return true;
}

function factorial(n: number): number {
    if (n <= 1) {
        return 1;
    }
    let result: number = 1;
    let i: number = 2;
    while (i <= n) {
        result = result * i;
        i = i + 1;
    }
    return result;
}

function main(): void {
    console.log("fib(0):", fibonacci(0));
    console.log("fib(1):", fibonacci(1));
    console.log("fib(5):", fibonacci(5));
    console.log("fib(10):", fibonacci(10));

    console.log("isPrime(2):", isPrime(2));
    console.log("isPrime(7):", isPrime(7));
    console.log("isPrime(4):", isPrime(4));
    console.log("isPrime(1):", isPrime(1));

    console.log("factorial(0):", factorial(0));
    console.log("factorial(5):", factorial(5));
    console.log("factorial(10):", factorial(10));
}
