A Rust arena allocator, focused on usability and speed\*. 


## Basic usage


```rust
use light_rc_arena::{Arena, ArenaRef};
use std::cell::Cell;

// Making an arena
type MyType = Cell<i32>;
let arena: Arena<MyType> = Arena::new();

// Allocating objects:
//
// These can be treated as &'arena MyType objects. You should wrap
// them in a Cell or RefCell to mutate.

let obj1: ArenaRef<MyType, _> = arena.alloc(Cell::new(0));
let obj2: ArenaRef<MyType, _> = arena.alloc(Cell::new(1));
let obj3: ArenaRef<MyType, _> = arena.alloc(Cell::new(2));

obj1.get(); //> 0
obj2.get(); //> 1

obj3.set(-99);
assert_eq!(obj3.get(), -99); //> -99

// You can clone the Arena or ArenaRefs as much as you like (just like an Rc)
// let _arena = arena.clone();
let _obj1 = obj1.clone();

// And even (not recommended) get an Arena from an ArenaRef
let _arena: Option<Arena<MyType>> = obj1.get_arena();

// But fair warning! An ArenaRef will NOT keep the arena alive!

// And it is YOUR RESPONSIBILITY to keep the arena alive
drop(arena);
drop(_arena);

// let value = &*obj1; // PANICS

// You can test if your arena is still valid:
let value: Option<&MyType> = obj1.try_get();
dbg!(value); // None

```



## Rationale 

1. It is your duty to keep your Arena alive. 
    * If you want a more _rusty_ arena, I recommend you use the [typed_arena](https://docs.rs/typed-arena/latest/typed_arena/) crate instead!
2. This crate is for small to moderately sized objects. **As such**, I am not going to over-engineer the allocator. 
    * If you need such objects they need manually memcopied, you should **NOT** be using an arena!
        * You should be putting such large objects (and creating them) on the heap yourself!
3. An allocator is not responsible for controlling your memory, only allocating it. **As such**, a standard reference is the only thing I am willing to handout. 
    * Anything else complicates things beyond the scope of an allocator.  








