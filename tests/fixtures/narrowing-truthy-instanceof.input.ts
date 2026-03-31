// Truthy and instanceof narrowing

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

// Truthy check on optional number
function truthyNumber(x: number | null): number {
  if (x) {
    return x;
  }
  return 0;
}

// Truthy check with else branch
function truthyWithElse(x: string | null): string {
  if (x) {
    return x;
  } else {
    return "default";
  }
}

// Null check (=== null narrowing)
function nullCheck(x: string | null): string {
  if (x === null) {
    return "null";
  }
  return x;
}

// typeof narrowing
function typeofNarrowing(x: string | number): string {
  if (typeof x === "string") {
    return x;
  }
  return x.toString();
}
