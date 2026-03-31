// Indexed access types: T['key']

interface Env {
  Bindings: {
    DB: string;
    CACHE: string;
  };
}

function getBindings(b: Env['Bindings']): Env['Bindings'] {
  return b;
}

// Nested indexed access
interface AppConfig {
  database: {
    host: string;
    port: number;
  };
  auth: {
    token: string;
  };
}

function getDbHost(config: AppConfig): AppConfig['database']['host'] {
  return config.database.host;
}

// Indexed access on array type
interface UserList {
  users: Array<{ name: string; age: number }>;
}

function getFirstUser(list: UserList): UserList['users'][number] {
  return list.users[0];
}

// Union key access
interface Settings {
  color: string;
  size: number;
  label: string;
}

type StringSettings = Settings['color' | 'label'];
