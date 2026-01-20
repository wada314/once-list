use ::allocator_api2::alloc::Allocator;
use ::allocator_api2::boxed::Box;
use ::std::cell::Cell;
use ::std::ptr::NonNull;

use crate::cons::Cons;
use crate::oncecell_ext::OnceCellExt;
use crate::OnceCell;

mod sealed {
    pub trait Sealed {}
}

/// A tail insertion slot.
///
/// This is a thin wrapper over an internal `OnceCell<Box<Cons<...>>>` so that mode APIs
/// don't leak internal node types in public signatures.
#[doc(hidden)]
#[derive(Clone)]
pub struct TailSlot<T: ?Sized, A: Allocator> {
    cell: OnceCell<Box<Cons<T, T, A>, A>>,
}

impl<T: ?Sized, A: Allocator> TailSlot<T, A> {
    pub(crate) fn new() -> Self {
        Self {
            cell: OnceCell::new(),
        }
    }

    pub(crate) fn get(&self) -> Option<&Box<Cons<T, T, A>, A>> {
        self.cell.get()
    }

    pub(crate) fn get_mut(&mut self) -> Option<&mut Box<Cons<T, T, A>, A>> {
        self.cell.get_mut()
    }

    pub(crate) fn set(&self, value: Box<Cons<T, T, A>, A>) -> Result<(), Box<Cons<T, T, A>, A>> {
        self.cell.set(value)
    }

    pub(crate) fn take(&mut self) -> Option<Box<Cons<T, T, A>, A>> {
        self.cell.take()
    }

    pub(crate) fn try_insert2(
        &self,
        value: Box<Cons<T, T, A>, A>,
    ) -> Result<&Box<Cons<T, T, A>, A>, (&Box<Cons<T, T, A>, A>, Box<Cons<T, T, A>, A>)> {
        self.cell.try_insert2(value)
    }
}

/// Cache mode for `OnceList` (e.g. tail cache, len cache).
///
/// This trait is **sealed**: downstream crates cannot implement it.
#[doc(hidden)]
pub trait CacheMode<T: ?Sized, A: Allocator>: sealed::Sealed + Clone {
    /// Returns cached length if available.
    fn cached_len(&self) -> Option<usize> {
        None
    }

    /// Returns the slot to start inserting from.
    fn start_slot<'a>(&'a self, head: &'a TailSlot<T, A>) -> &'a TailSlot<T, A>;

    /// Called after a push successfully inserted a node.
    fn on_push_success(&self, next_slot: &TailSlot<T, A>);

    /// Called after a remove successfully removed a node.
    fn on_remove_success(&self) {}

    /// Called when the list is cleared.
    fn on_clear(&self) {}

    /// Called when list structure may change via `&mut self` methods (e.g. remove), to invalidate caches as needed.
    fn invalidate(&self);
}

/// No caching. This is the original behavior.
#[derive(Clone, Copy)]
pub struct NoTail;

impl sealed::Sealed for NoTail {}

impl<T: ?Sized, A: Allocator> CacheMode<T, A> for NoTail {
    fn start_slot<'a>(&'a self, head: &'a TailSlot<T, A>) -> &'a TailSlot<T, A> {
        head
    }

    fn on_push_success(&self, _next_slot: &TailSlot<T, A>) {}

    fn invalidate(&self) {}
}

/// Tail caching mode (single-thread oriented).
///
/// This caches the *next insertion slot* (`node.next`), not the tail node itself.
///
/// IMPORTANT: This never caches `&head` (which would become dangling after a move),
/// it only caches pointers into heap-allocated nodes.
pub struct WithTail<T: ?Sized, A: Allocator> {
    next_slot: Cell<Option<SlotPtr<T, A>>>,
}

type SlotPtr<T, A> = NonNull<TailSlot<T, A>>;

impl<T: ?Sized, A: Allocator> Clone for WithTail<T, A> {
    fn clone(&self) -> Self {
        // Do NOT clone the pointer; it would point into the other list.
        Self::new()
    }
}

impl<T: ?Sized, A: Allocator> sealed::Sealed for WithTail<T, A> {}

impl<T: ?Sized, A: Allocator> CacheMode<T, A> for WithTail<T, A> {
    fn start_slot<'a>(&'a self, head: &'a TailSlot<T, A>) -> &'a TailSlot<T, A> {
        if let Some(p) = self.next_slot.get() {
            let slot = unsafe { p.as_ref() };
            // Fast-path: if the cached slot is still empty, use it.
            if slot.get().is_none() {
                return slot;
            }
        }
        head
    }

    fn on_push_success(&self, next_slot: &TailSlot<T, A>) {
        self.next_slot.set(Some(NonNull::from(next_slot)));
    }

    fn invalidate(&self) {
        self.next_slot.set(None);
    }
}

impl<T: ?Sized, A: Allocator> WithTail<T, A> {
    pub(crate) fn new() -> Self {
        Self {
            next_slot: Cell::new(None),
        }
    }
}

/// Tail + len caching mode (single-thread oriented).
pub struct WithTailLen<T: ?Sized, A: Allocator> {
    next_slot: Cell<Option<SlotPtr<T, A>>>,
    len: Cell<usize>,
}

impl<T: ?Sized, A: Allocator> Clone for WithTailLen<T, A> {
    fn clone(&self) -> Self {
        // Do NOT clone the pointer; it would point into the other list.
        // Cloning len is fine (it's a value).
        Self {
            next_slot: Cell::new(None),
            len: Cell::new(self.len.get()),
        }
    }
}

impl<T: ?Sized, A: Allocator> sealed::Sealed for WithTailLen<T, A> {}

impl<T: ?Sized, A: Allocator> CacheMode<T, A> for WithTailLen<T, A> {
    fn cached_len(&self) -> Option<usize> {
        Some(self.len.get())
    }

    fn start_slot<'a>(&'a self, head: &'a TailSlot<T, A>) -> &'a TailSlot<T, A> {
        if let Some(p) = self.next_slot.get() {
            let slot = unsafe { p.as_ref() };
            if slot.get().is_none() {
                return slot;
            }
        }
        head
    }

    fn on_push_success(&self, next_slot: &TailSlot<T, A>) {
        self.len.set(self.len.get() + 1);
        self.next_slot.set(Some(NonNull::from(next_slot)));
    }

    fn on_remove_success(&self) {
        self.len.set(self.len.get() - 1);
    }

    fn on_clear(&self) {
        self.len.set(0);
        self.next_slot.set(None);
    }

    fn invalidate(&self) {
        // Keep `len` (it is still correct); only invalidate tail slot.
        self.next_slot.set(None);
    }
}

impl<T: ?Sized, A: Allocator> WithTailLen<T, A> {
    pub(crate) fn new() -> Self {
        Self {
            next_slot: Cell::new(None),
            len: Cell::new(0),
        }
    }
}
