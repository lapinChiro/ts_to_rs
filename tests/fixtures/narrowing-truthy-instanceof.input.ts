// Phase A-2: truthy and instanceof narrowing

// Case 1: truthy check on nullable → if let Some
function truthyCheck(x: string | null): void {
    if (x) {
        console.log(x);
    }
}

// Case 2: instanceof on any parameter with locally-defined class
class MyError {
    message: string;
    constructor(msg: string) {
        this.message = msg;
    }
}

function instanceofAny(x: any): void {
    if (x instanceof MyError) {
        console.log(x.message);
    }
}
