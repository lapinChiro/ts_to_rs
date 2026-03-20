// Trait type coercion: auto-adjusting expressions for trait type positions

// Pure method interface → trait
interface Greeter {
    greet(): string;
}

// Function taking trait param: &dyn Trait
function useGreeter(g: Greeter): string {
    return g.greet();
}

// Function returning trait: Box<dyn Trait>
function createGreeter(): Greeter {
    return null as any;
}

// Case 1: Passing Box<dyn Trait> variable to &dyn Trait parameter → needs &*
function testBoxToRef(): void {
    const g: Greeter = createGreeter();
    useGreeter(g);
}

// Case 2: Concrete type assigned to trait-typed variable → needs Box::new()
class MyGreeter implements Greeter {
    greet(): string {
        return "hello";
    }
}

function testConcreteToBox(): void {
    const g: Greeter = new MyGreeter();
}

// Case 3: Return already-Box value from Box<dyn Trait> function (no extra wrapping)
function testReturn(): Greeter {
    const g: Greeter = createGreeter();
    return g;
}

// Non-trait interface (fields only) should NOT get coercion
interface Point {
    x: number;
    y: number;
}

function usePoint(p: Point): number {
    return p.x;
}
