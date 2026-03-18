function main(): void {
    // Postfix increment with local variable
    let count: number = 0;
    count++;
    console.log("after ++:", count);

    // Postfix decrement
    count--;
    console.log("after --:", count);

    // Multiple increments
    let x: number = 5;
    x++;
    x++;
    x++;
    console.log("x after 3 increments:", x);

    // Decrement in loop body
    let n: number = 3;
    while (n > 0) {
        console.log("n:", n);
        n--;
    }
}
