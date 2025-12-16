use std::{
    cell::{Cell, UnsafeCell}, mem::MaybeUninit, ops::Deref, ptr::addr_of_mut, rc::{Rc, Weak}
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
struct ArenaInner<T, const N: usize> {
    tail: Cell<*const Segment<T, N>>,

    _head: Box<Segment<T, N>>,
}

impl<T, const N: usize> ArenaInner<T, N> {
    fn alloc(&self, cont: T) -> *mut T {
        let tail = unsafe { &*self.tail.get() };

        if tail.length.get() >= N {
            let segment = Segment::new();

            // This looks evil, but the box means this is valid*
            self.tail.set(&*segment as *const Segment<T, N>);
            tail.next.set(Some(segment));
        }

        let tail = unsafe { &*self.tail.get() };

        let old_length = tail.length.get();
        let contents = unsafe {
            let inner = &tail.data[old_length];

            // SAFETY: since it has not been "allocated" in the arena,
            //         it has not been shared, so free to write over.

            (&mut *inner.get()).write(cont)
        };

        tail.length.set(old_length + 1);
        contents
    }
}

pub struct Arena<T : Sized, const N: usize = 32> {
    inner: Rc<ArenaInner<T, N>>,
}

pub struct ArenaRef<T : Sized, const N: usize> {
    arena: Weak<ArenaInner<T, N>>,

    // SAFETY: ptr MUST be contained within the tree of parent! And 
    //         it will be valid aslong as arena is valid
    ptr: *const T,
}

impl<T, const N: usize> ArenaRef<T, N> {
    fn try_get(&self) -> Option<&T> {
        // SAFETY: According to the 'weak_count' docs: `If no strong pointers remain, this will
        //         return zero.` So this is a valid check for if the arena is still valid
        if self.arena.weak_count() == 0 {
            None
        } else {
            Some(unsafe {&*self.ptr})
        }
    }
}


impl<T, const N: usize> Deref for ArenaRef<T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.try_get().expect("The arena assosiated with this value is no longer valid!")
    }
}

impl<T, const N: usize> Arena<T, N> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_usage() {
        let arena: Arena<Cell<u32>, 8> = Arena::new();
        for i in 0..128{
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
}
