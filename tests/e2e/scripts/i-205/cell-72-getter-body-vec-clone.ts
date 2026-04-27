class Bag { _items: number[] = [1, 2, 3]; get items(): number[] { return this._items; } }
const b = new Bag();
console.log(b.items);
