// B12: `!x` where x is Array or Function (always truthy in JS).
console.log(![]);        // false (empty array is truthy)
console.log(![1, 2]);    // false
console.log(!(() => 0)); // false (functions always truthy)

function f(arr: number[]): boolean { return !arr; }
console.log(f([]));      // false
console.log(f([1, 2]));  // false
