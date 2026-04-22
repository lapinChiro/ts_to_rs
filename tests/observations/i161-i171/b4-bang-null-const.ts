// B4: `!null`, `!undefined`, `!0`, `!""`, `!false`, `!true` constant folds.
console.log(!null);       // true
console.log(!undefined);  // true
console.log(!0);          // true
console.log(!"");         // true
console.log(!false);      // true
console.log(!true);       // false
console.log(!1);          // false
console.log(!"x");        // false
