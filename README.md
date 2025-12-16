A Rust arena allocator focused on usability and speed.



## Basic usage

```rust
use light_rc_arena::{Arena, ArenaRef};
use std::cell::Cell;

// Creating an arena.
type MyType = Cell<i32>;
let arena: Arena<MyType> = Arena::new();

// Allocating objects:
//
// These can be treated as &'arena MyType references. You should wrap
// them in a Cell or RefCell to mutate.

let obj1: ArenaRef<MyType, _> = arena.alloc(Cell::new(0));
let obj2: ArenaRef<MyType, _> = arena.alloc(Cell::new(1));
let obj3: ArenaRef<MyType, _> = arena.alloc(Cell::new(2));

obj1.get(); //> 0
obj2.get(); //> 1

obj3.set(-99);
assert_eq!(obj3.get(), -99); //> -99

// You can clone the Arena or ArenaRefs as much as you like, just like an Rc.
let arena2 = arena.clone();
let _ = obj1.clone();

// You can even (not recommended) get an Arena from an ArenaRef.
// In this case, it will be None if the Arena has been dropped.
let arena3: Option<Arena<MyType>> = obj1.get_arena();

// But fair warning: an ArenaRef will NOT keep the Arena alive!

// It is YOUR RESPONSIBILITY to keep the Arena alive.
drop(arena);
drop(arena2);
drop(arena3);

// let value = &*obj1; // panics

// You can test if your Arena is still alive:
let value: Option<&MyType> = obj1.try_get();
dbg!(value); //> None
```



## Rationale

1. It is your responsibility to keep your `Arena` alive.
    * If you want a more _Rusty_ arena, I recommend you use the
      [`typed_arena`](https://docs.rs/typed-arena/latest/typed_arena/) crate
      instead.
2. This crate is for small to moderately-sized objects. For that reason, I am
   not going to over-engineer the allocator.
    * If you need objects so large that they have to be manually memcopied, you
      should **NOT** be using an arena!
        * You should allocate such large objects (and construct them) on the
          heap yourself.
3. An allocator is not responsible for controlling your memory, only allocating
   it. For that reason, a standard reference is the only thing I am willing to
   hand out.
    * Anything else complicates things beyond the scope of an allocator.

