function doSomething(): void {
  let x: number = 1;
}

function runCallback(cb: (x: number) => void): void {
  cb(42);
}
