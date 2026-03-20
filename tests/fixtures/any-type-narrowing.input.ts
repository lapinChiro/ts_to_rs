// any type lazy materialization: typeof on any generates enum

// Case 1: typeof on any parameter (regular function) → auto-generated enum + if let
function processAny(x: any): void {
    if (typeof x === "string") {
        console.log(x);
    }
}

// Case 2: no typeof/instanceof on any → keep as serde_json::Value
function passthrough(x: any): void {
    console.log(x);
}

// Case 3: typeof on any parameter (arrow function, block body)
const checkArrow = (x: any): void => {
    if (typeof x === "string") {
        console.log(x);
    }
};

// Case 4: typeof on any local variable (non-null init to avoid null→None issue)
function localAny(input: any): void {
    const data: any = input;
    if (typeof data === "string") {
        console.log(data);
    }
}

// Case 5: typeof on any parameter (arrow function, expression body)
const exprArrow = (x: any): number => typeof x === "string" ? x.length : 0;
