// Class inheritance patterns

class Animal {
  name: string;
  constructor(name: string) {
    this.name = name;
  }
  speak(): string {
    return `${this.name}`;
  }
}

class Dog extends Animal {
  constructor(name: string) {
    super(name);
  }
  bark(): string {
    return this.speak() + " barks";
  }
}

// Method override
class Cat extends Animal {
  constructor(name: string) {
    super(name);
  }
  speak(): string {
    return `${this.name} meows`;
  }
}

// super.method() call
class Vehicle {
  speed: number;
  constructor(speed: number) {
    this.speed = speed;
  }
  describe(): string {
    return `speed: ${this.speed}`;
  }
}

class Car extends Vehicle {
  brand: string;
  constructor(brand: string, speed: number) {
    super(speed);
    this.brand = brand;
  }
  describe(): string {
    return `${this.brand} - ${super.describe()}`;
  }
}

// Additional field in subclass
class Truck extends Vehicle {
  payload: number;
  constructor(speed: number, payload: number) {
    super(speed);
    this.payload = payload;
  }
  describe(): string {
    return `truck payload: ${this.payload}`;
  }
}
