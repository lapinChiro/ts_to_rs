function main(): void {
    const nums: number[] = [1, 2, 3, 4, 5];

    // map: verify by reducing the result
    const doubled = nums.map((x: number): number => x * 2);
    console.log("map length:", doubled.length);
    const doubledSum = doubled.reduce((acc: number, x: number): number => acc + x, 0);
    console.log("map sum:", doubledSum);

    // filter: verify length and sum
    const evens = nums.filter((x: number): boolean => x % 2 === 0);
    console.log("filter length:", evens.length);
    const evensSum = evens.reduce((acc: number, x: number): number => acc + x, 0);
    console.log("filter sum:", evensSum);

    // some
    console.log("some >3:", nums.some((x: number): boolean => x > 3));
    console.log("some >10:", nums.some((x: number): boolean => x > 10));

    // every
    console.log("every >0:", nums.every((x: number): boolean => x > 0));
    console.log("every >3:", nums.every((x: number): boolean => x > 3));

    // reduce
    const sum = nums.reduce((acc: number, x: number): number => acc + x, 0);
    console.log("reduce sum:", sum);

    // forEach
    let total: number = 0;
    nums.forEach((x: number): void => {
        total = total + x;
    });
    console.log("forEach total:", total);

    // indexOf
    console.log("indexOf 3:", nums.indexOf(3));
    console.log("indexOf 99:", nums.indexOf(99));

    // slice: verify length and sum
    const sliced = nums.slice(1, 3);
    console.log("slice length:", sliced.length);
    const slicedSum = sliced.reduce((acc: number, x: number): number => acc + x, 0);
    console.log("slice sum:", slicedSum);

    // sort: verify by reducing sorted array
    const unsorted: number[] = [3, 1, 4, 1, 5];
    unsorted.sort();
    const sortCheck = unsorted.some((x: number): boolean => x === 1);
    console.log("sort contains 1:", sortCheck);
    console.log("sort length:", unsorted.length);

    // find: returns T | undefined in TS, Option<T> in Rust
    const firstOver3 = nums.find((x: number): boolean => x > 3);
    if (firstOver3 !== undefined) {
        console.log("find >3:", firstOver3);
    } else {
        console.log("find >3: none");
    }
    const firstOver100 = nums.find((x: number): boolean => x > 100);
    if (firstOver100 !== undefined) {
        console.log("find >100:", firstOver100);
    } else {
        console.log("find >100: none");
    }

    // filter with captured variable (threshold) — verifies filter closure
    // receives &T (Rust Iterator::filter semantics) but body uses T by value
    const threshold: number = 2;
    const above = nums.filter((x: number): boolean => x > threshold);
    console.log("filter >threshold length:", above.length);
    const aboveSum = above.reduce((acc: number, x: number): number => acc + x, 0);
    console.log("filter >threshold sum:", aboveSum);
}
