// Duplicate interface - identical body (should be error)
interface MyInterface {
  name: string;
  age: number;
}

// Duplicate interface - identical body (should be error)
interface MyInterface {
  name: string;
  age: number;
}

// Duplicate interface name - different body (should be warning)
interface MyInterface2 {
  id: number;
}

// Classes implementing the interfaces
class PersonA implements MyInterface {
  name = "Alice";
  age = 30;
}

class PersonB implements MyInterface {
  name = "Bob";
  age = 25;
}

class Widget implements MyInterface2 {
  id = 1;
}
