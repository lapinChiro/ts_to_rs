// I-177-D Matrix Cell #6: Primary(InstanceofGuard) + closure-reassign + body read
//
// 案 C target: TypeResolver narrowed_type for instanceof Foo narrow inside
// cons-span returns Some(Named { name: "Foo" }) even when closure-reassign exists.

class Dog {
    bark(): string { return "woof"; }
}

function f(animal: Dog | null): string {
    let last: string = "init";
    const reset = () => { animal = null; };
    if (animal instanceof Dog) {
        last = animal.bark();  // Primary narrow read: animal: Dog (case-C target)
    }
    reset();                   // mutates outer animal; runtime: animal = null
    return last;
}

function main(): void {
    console.log(f(new Dog())); // "woof"
    console.log(f(null));      // "init"
}
