// A5: `obj.field &&= y`.
// Should compile member-LHS just like ident-LHS.
const obj: { x: number; y: number | null } = { x: 5, y: null };
obj.x &&= 10;
obj.y &&= 10;
console.log(obj.x, obj.y);  // 10 null
