// Void type patterns

function doSomething(): void {
  let x: number = 1;
}

function runCallback(cb: (x: number) => void): void {
  cb(42);
}

// void in union type
type MaybeVoid = string | void;

function maybeReturn(flag: boolean): string | void {
  if (flag) {
    return "hello";
  }
}

// Promise<void>
async function runAsync(): Promise<void> {
  console.log("done");
}

// void function with side effects
function printAll(items: string[]): void {
  for (const item of items) {
    console.log(item);
  }
}

// Callback returning void in interface
interface EventHandler {
  onClick: (event: string) => void;
  onClose: () => void;
}
