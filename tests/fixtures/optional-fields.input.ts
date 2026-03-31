// Optional fields: optional properties, nullable types, access patterns

interface Config {
  host: string;
  port: number;
  debug?: boolean;
  label: string | null;
}

// Interface with multiple optional fields
interface UserProfile {
  name: string;
  email?: string;
  age?: number;
  bio?: string;
}

// Function accessing optional fields with defaults
function getPort(config: Config): number {
  return config.port;
}

// Accessing optional field with nullish coalescing
function isDebug(config: Config): boolean {
  return config.debug ?? false;
}

// Optional fields in nested structure
interface Address {
  street: string;
  city: string;
  zip?: string;
}

interface Person {
  name: string;
  address?: Address;
}
