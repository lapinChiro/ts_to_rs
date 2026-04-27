class Config { static _v = 0; static get x(): number { return Config._v; } static set x(v: number) { Config._v = v * 10; } }
Config.x = 5;
console.log(Config.x);
