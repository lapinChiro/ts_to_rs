function main(): void {
    const arr: number[] = [1, 2, 3, 4, 5];
    console.log("length:", arr.length);

    // index access
    console.log("first:", arr[0]);
    console.log("last:", arr[4]);

    // index with variable
    const idx: number = 2;
    console.log("middle:", arr[idx]);

    // push and check
    const arr2: number[] = [10, 20];
    arr2.push(30);
    console.log("after push length:", arr2.length);
}
