// Mix of supported and unsupported syntax
interface User {
  name: string;
  age: number;
}

// Unsupported: export default expression
export default 42;

function greet(user: User): string {
  return `Hello, ${user.name}`;
}

// Unsupported: decorator (if available)
// Note: decorators are unsupported and should be reported

// Unsupported: namespace
namespace MyNamespace {
  export function helper(): string {
    return "help";
  }
}
