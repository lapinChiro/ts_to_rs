function demo(x: number, name: string): number {
    console.log("start");
    console.log(x);
    console.error(name);
    console.warn(x, name);
    console.log();
    return x;
}
