#![doc = include_str!("../README.md")]
#![allow(unused)]

use std::{
    cell::{Cell, UnsafeCell},
    fmt::{Debug, Display, Formatter},
    mem::MaybeUninit,
    ops::Deref,
    ptr::addr_of_mut,
    rc::{Rc, Weak},
};

struct Segment<T, const N: usize> {
    length: Cell<usize>,

    // Note: This is only used for cleanup
    next: Cell<Option<Box<Segment<T, N>>>>,

    data: [UnsafeCell<MaybeUninit<T>>; N],
}

impl<T, const N: usize> Segment<T, N> {
    fn new() -> Box<Segment<T, N>> {
        // Create the Segment on the heap. As larger N values can oversaturate the stack
        unsafe {
            let layout = std::alloc::Layout::new::<Segment<T, N>>();
            let ptr = std::alloc::alloc(layout) as *mut Segment<T, N>;

            // Handle OOMs
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            addr_of_mut!((*ptr).length).write(Cell::new(0));
            addr_of_mut!((*ptr).next).write(Cell::new(None));

            // SAFETY: `data` does NOT need initialized due to it containing MaybeUninit and the
            //         length being 0.

            Box::from_raw(ptr)
        }
    }
}

impl<T, const N: usize> Drop for Segment<T, N> {
    fn drop(&mut self) {
        unsafe {
            for i in 0..self.length.get() {
                self.data[i].get_mut().assume_init_drop();
            }
        }
    }
}

use std::cell::RefCell;
/// A reference to a value within an [`Arena`]. It can be treated like any other reference type.
///
/// But, it is exclusivly read only. However you can still use a [`Cell`] or [`RefCell`] for interior
/// mutability.
pub struct ArenaRef<T: Sized, const N: usize> {
    arena: Weak<ArenaInner<T, N>>,

    // SAFETY: ptr MUST be contained within the tree of parent! And
    //         it will be valid as long as arena is valid
    ptr: *const T,
}

impl<T, const N: usize> ArenaRef<T, N> {
    ///  Try to retrieve the contained value.
    ///
    ///  [`None`] corresponds to the parent [`Arena`] no longer existing
    pub fn try_get(&self) -> Option<&T> {
        // SAFETY: According to the 'weak_count' docs: `If no strong pointers remain, this will
        //         return zero.` So this is a valid check for if the arena is still valid
        if self.arena.weak_count() == 0 {
            None
        } else {
            Some(unsafe { &*self.ptr })
        }
    }

    ///  Try to retrive the parent [`Arena`]. Returns [`None`] when it is no longer alive,
    pub fn get_arena(&self) -> Option<Arena<T, N>> {
        self.arena.upgrade().map(|inner| Arena { inner })
    }


    /// Test if two [`ArenaRef`]s are pointing to the same values in the same [`Arena`]s.
    pub fn ptr_eq(&self, other : &Self) -> bool {
        self.ptr.eq(&other.ptr) && self.get_arena().eq(&other.get_arena())
    }

}

impl<T, const N: usize> Clone for ArenaRef<T, N> {
    fn clone(&self) -> Self {
        ArenaRef {
            arena: self.arena.clone(),
            ptr: self.ptr,
        }
    }
}

impl<T, const N: usize> Deref for ArenaRef<T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.try_get()
            .expect("The arena assosiated with this value is no longer valid!")
    }
}

impl<T: Debug, const N: usize> Debug for ArenaRef<T, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.try_get() {
            Some(value) => f.debug_tuple("ArenaRef").field(value).finish(),
            None => f.debug_tuple("ArenaRef").field(&"<dead arena>").finish(),
        }
    }
}

impl<T: Display, const N: usize> Display for ArenaRef<T, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.try_get() {
            Some(value) => std::fmt::Display::fmt(value, f),
            None => write!(f, "<dead arena reference>"),
        }
    }
}

struct ArenaInner<T, const N: usize> {
    tail: Cell<*const Segment<T, N>>,

    _head: Box<Segment<T, N>>,
}

impl<T, const N: usize> ArenaInner<T, N> {
    fn alloc(&self, cont: T) -> *mut T {
        let tail = unsafe { &*self.tail.get() };

        if tail.length.get() >= N {
            let segment = Segment::new();

            // This looks evil, but the box means this is valid
            self.tail.set(&*segment as *const Segment<T, N>);
            tail.next.set(Some(segment));
        }

        let tail = unsafe { &*self.tail.get() };

        let old_length = tail.length.get();
        let contents = unsafe {
            let inner = &tail.data[old_length];

            // SAFETY: since it has not been "allocated" in the arena,
            //         it has not been shared, so it is free to write over.

            (&mut *inner.get()).write(cont)
        };

        tail.length.set(old_length + 1);
        contents
    }
}

/// A typed memory arena that you can pass like an [`Rc`].
///
///
/// Example:
/// ```
/// use light_rc_arena::*;
///
/// let arena = Arena::<i32>::new();
///
/// // Still a reference to the same arena
/// let arena_copy = arena.clone();
///
/// let x : ArenaRef<i32, _> = arena_copy.alloc(-1);
/// dbg!(*x); // -1
///
/// ```
///
pub struct Arena<T: Sized, const N: usize = 64> {
    inner: Rc<ArenaInner<T, N>>,
}

impl<T, const N: usize> Arena<T, N> {
    /// Create a new Arena
    pub fn new() -> Arena<T, N> {
        assert!(N > 0, "Using zero for segment size is illegal!");

        let new_segment = Segment::new();
        let inner = Rc::new(ArenaInner {
            // Temp value
            tail: Cell::from(&*new_segment as *const Segment<T, N>),
            _head: new_segment,
        });

        Arena { inner }
    }

    /// Move an object into the arena, and return a [`ArenaRef`] to its new location.
    #[inline]
    pub fn alloc(&self, cont: T) -> ArenaRef<T, N> {
        ArenaRef {
            arena: Rc::downgrade(&self.inner),
            ptr: self.inner.alloc(cont),
        }
    }
}

impl<T, const N: usize> Clone for Arena<T, N> {
    fn clone(&self) -> Self {
        Arena {
            inner: self.inner.clone(),
        }
    }
}

impl<T, const N: usize> PartialEq for Arena<T, N> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn basic_usage() {
        let arena: Arena<Cell<u32>, 8> = Arena::new();
        for i in 0..128 {
            let r = arena.alloc(Cell::new(i));
            // Test value
            assert_eq!(r.get(), i);

            // Test mutibility
            assert_eq!(r.replace(1), i);

            // Ensure set
            assert_eq!(r.get(), 1);
        }

        let r = arena.alloc(Cell::new(1));
        assert_eq!(r.get(), 1);

        drop(arena);

        assert_eq!(r.try_get(), None);
    }

    #[test]
    fn usage_guide_from_readme() {
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

        assert_eq!(obj1.get(), 0); //> 0
        assert_eq!(obj2.get(), 1); //> 1

        obj3.set(-99);
        assert_eq!(obj3.get(), -99); //> -99

        // You can clone the Arena or ArenaRefs as much as you like (just like an Rc)
        let arena2 = arena.clone();
        let _obj1 = obj1.clone();

        // And you can even get an Arena from an ArenaRef (though this is not recommended).
        let arena3: Option<Arena<MyType>> = obj1.get_arena();

        // But fair warning! An ArenaRef will NOT keep the arena alive!

        // And it is YOUR RESPONSIBILITY to keep the arena alive
        drop(arena);
        drop(arena2);
        drop(arena3);

        // let value = &*obj1; // PANICS

        // You can test if your arena is still valid:
        let value: Option<&MyType> = obj1.try_get();
        assert_eq!(value, None)
    }
}
