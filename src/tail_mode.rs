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
/// This is a thin wrapper over an internal `OnceCell<Box<Cons<...>>>` so that the tail mode API
/// doesn't leak internal node types in public signatures.
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

/// Tail selection strategy for `OnceList`.
///
/// This trait is **sealed**: downstream crates cannot implement it.
#[doc(hidden)]
pub trait TailMode<T: ?Sized, A: Allocator>: sealed::Sealed + Clone {
    fn start_slot<'a>(&'a self, head: &'a TailSlot<T, A>) -> &'a TailSlot<T, A>;
    fn on_push_success(&self, next_slot: &TailSlot<T, A>);
    fn invalidate(&self);
}

/// No tail caching. This is the original behavior.
#[derive(Clone, Copy)]
pub struct NoTail;

impl sealed::Sealed for NoTail {}

impl<T: ?Sized, A: Allocator> TailMode<T, A> for NoTail {
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

impl<T: ?Sized, A: Allocator> TailMode<T, A> for WithTail<T, A> {
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

