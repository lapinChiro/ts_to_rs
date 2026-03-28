// T1: Vec→Array method/field mapping tests
// Tests that Vec<T> method arguments receive the element type T as expected type,
// and that Array fields (like length) are accessible on Vec types.

interface TodoItem {
    title: string;
    done: boolean;
}

// S4: push should propagate element type to argument
function addTodo(items: TodoItem[]): void {
    items.push({ title: "Learn Rust", done: false });
}

// S10+S1: map callback parameter should be inferred as element type
function getTitles(items: TodoItem[]): string[] {
    return items.map(item => item.title);
}

// S10: filter callback parameter should be inferred as element type
function getActiveTodos(items: TodoItem[]): TodoItem[] {
    return items.filter(item => !item.done);
}

// S10: forEach callback parameter should be inferred
function printTodos(items: TodoItem[]): void {
    items.forEach(item => {
        console.log(item.title);
    });
}

// Field access: arr.length should resolve to f64
function getTodoCount(items: TodoItem[]): number {
    return items.length;
}
