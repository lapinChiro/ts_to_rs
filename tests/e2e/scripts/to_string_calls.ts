// I-67: closure with String parameter - .to_string() at call site
function main(): void {
    const shout = (msg: string): string => {
        return msg + "!";
    };
    console.log(shout("wow"));
    console.log(shout("hello"));
}
