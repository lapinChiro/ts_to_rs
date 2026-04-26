// I-177-D Matrix Cell #18: Primary(OptChainInvariant) + closure-reassign + body read
//
// 案 C target: TypeResolver narrowed_type for OptChain `x?.prop !== undefined`
// narrow inside cons-span returns Some(T) for the base x, even when closure-
// reassign exists. (T7, T12 narrow class)

interface Config {
    name: string;
}

function f(c: Config | null): string {
    let last: string = "init";
    const reset = () => { c = null; };
    if (c?.name !== undefined) {
        last = c.name;          // Primary narrow read: c: Config (OptChainInvariant)
    }
    reset();                    // mutates outer c; runtime: c = null
    return last;
}

function main(): void {
    console.log(f({ name: "alice" })); // "alice"
    console.log(f(null));               // "init"
}
