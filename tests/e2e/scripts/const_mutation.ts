function testPush(): string {
    const arr: number[] = [1, 2, 3];
    arr.push(4);
    arr.push(5);
    return "pushed ok";
}

function testSort(): string {
    const nums: number[] = [3, 1, 2];
    nums.sort();
    nums.reverse();
    return "sorted ok";
}

function main(): void {
    console.log(testPush());
    console.log(testSort());
}
