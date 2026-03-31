// Type alias and utility type patterns

// Type alias referencing a registered type (simple ref)
interface Config {
  host: string;
  port: number;
  debug: boolean;
}

type AppConfig = Config;

// Type alias with Partial utility type
type OptionalConfig = Partial<Config>;

// Intersection of type literal and type ref
type ExtendedConfig = Config & { extra: string };

// Intersection of two type literals
type Combined = { a: number } & { b: string };

// Required<T>
interface PartialUser {
  name?: string;
  age?: number;
}

type FullUser = Required<PartialUser>;

// Pick<T, K>
type HostOnly = Pick<Config, "host">;

// Omit<T, K>
type WithoutDebug = Omit<Config, "debug">;

// Readonly<T>
type ReadonlyConfig = Readonly<Config>;

// Record<K, V>
type StringMap = Record<string, number>;

// Function using utility types
function getHost(config: HostOnly): string {
  return config.host;
}

function createConfig(host: string, port: number): AppConfig {
  return { host, port, debug: false };
}
