function main(): void {
    // Postfix: returns old value
    let i: number = 0;
    const oldVal: number = i++;
    console.log("postfix old:", oldVal);
    console.log("postfix new:", i);

    // Prefix: returns new value
    let j: number = 10;
    const newVal: number = ++j;
    console.log("prefix new:", newVal);
    console.log("prefix j:", j);

    // Postfix decrement
    let k: number = 5;
    const oldK: number = k--;
    console.log("dec old:", oldK);
    console.log("dec new:", k);

    // In expression context (array index)
    const arr: number[] = [10, 20, 30, 40];
    let idx: number = 0;
    console.log("arr[idx++]:", arr[idx++]);
    console.log("idx after:", idx);
    console.log("arr[idx++]:", arr[idx++]);
    console.log("idx after:", idx);
}
