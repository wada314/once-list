use ::allocator_api2::alloc::Allocator;
use ::allocator_api2::alloc::Global;
use ::allocator_api2::boxed::Box;
use ::std::cell::Cell;
use ::std::ptr::NonNull;

use crate::cons::Cons;
use crate::once_list::OnceListCore;
use crate::oncecell_ext::OnceCellExt;
use crate::OnceCell;

mod sealed {
    pub trait Sealed {}
}

/// A "next node" slot (a thin wrapper around an internal `OnceCell`).
///
/// This type is used for:
/// - The list's **head slot** (a slot that points to the first node)
/// - Each node's **next slot** (`node.next`)
/// - Optional **tail insertion caching** (caching a pointer to some node's `next` slot)
///
/// Caching focuses on the tail insertion hot path, but the slot itself is conceptually "the next slot"
/// in a singly-linked list.
#[doc(hidden)]
#[derive(Clone)]
pub struct NextSlot<T: ?Sized, A: Allocator> {
    cell: OnceCell<Box<Cons<T, T, A>, A>>,
}

impl<T: ?Sized, A: Allocator> NextSlot<T, A> {
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

    /// Returns a cached tail insertion slot, if available.
    ///
    /// Returning `None` means the caller should fall back to scanning from the head.
    fn tail_slot_opt<'a>(&'a self) -> Option<&'a NextSlot<T, A>> {
        None
    }

    /// Called after a push successfully inserted a node.
    fn on_push_success(&self, next_slot: &NextSlot<T, A>);

    /// Called after a remove successfully removed a node.
    fn on_remove_success(&self) {}

    /// Called when the list is cleared.
    fn on_clear(&self) {}

    /// Called when list structure may change via `&mut self` methods (e.g. remove), to invalidate caches as needed.
    fn invalidate(&self);
}

/// No caching. This is the original behavior.
#[derive(Clone, Copy)]
pub struct NoCache;

impl sealed::Sealed for NoCache {}

impl<T: ?Sized, A: Allocator> CacheMode<T, A> for NoCache {
    fn on_push_success(&self, _next_slot: &NextSlot<T, A>) {}

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

type SlotPtr<T, A> = NonNull<NextSlot<T, A>>;

impl<T: ?Sized, A: Allocator> Clone for WithTail<T, A> {
    fn clone(&self) -> Self {
        // Do NOT clone the pointer; it would point into the other list.
        Self::new()
    }
}

impl<T: ?Sized, A: Allocator> sealed::Sealed for WithTail<T, A> {}

impl<T: ?Sized, A: Allocator> CacheMode<T, A> for WithTail<T, A> {
    fn tail_slot_opt<'a>(&'a self) -> Option<&'a NextSlot<T, A>> {
        if let Some(p) = self.next_slot.get() {
            let slot = unsafe { p.as_ref() };
            // Fast-path: if the cached slot is still empty, use it.
            if slot.get().is_none() {
                return Some(slot);
            }
        }
        None
    }

    fn on_push_success(&self, next_slot: &NextSlot<T, A>) {
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

impl<T: ?Sized> WithTail<T, Global> {
    /// Creates a new empty list using this cache mode.
    pub fn new_list() -> OnceListCore<T, Global, WithTail<T, Global>> {
        OnceListCore {
            head_slot: NextSlot::new(),
            alloc: Global,
            cache_mode: WithTail::new(),
        }
    }
}

impl<T: ?Sized, A: Allocator> WithTail<T, A> {
    /// Creates a new empty list with the given allocator using this cache mode.
    pub fn new_list_in(alloc: A) -> OnceListCore<T, A, WithTail<T, A>> {
        OnceListCore {
            head_slot: NextSlot::new(),
            alloc,
            cache_mode: WithTail::new(),
        }
    }
}

/// Len-only caching mode (single-thread oriented).
pub struct WithLen<T: ?Sized, A: Allocator> {
    len: Cell<usize>,
    _phantom: ::std::marker::PhantomData<fn(&T, &A)>,
}

impl<T: ?Sized, A: Allocator> Clone for WithLen<T, A> {
    fn clone(&self) -> Self {
        Self {
            len: Cell::new(self.len.get()),
            _phantom: ::std::marker::PhantomData,
        }
    }
}

impl<T: ?Sized, A: Allocator> sealed::Sealed for WithLen<T, A> {}

impl<T: ?Sized, A: Allocator> CacheMode<T, A> for WithLen<T, A> {
    fn cached_len(&self) -> Option<usize> {
        Some(self.len.get())
    }

    fn on_push_success(&self, _next_slot: &NextSlot<T, A>) {
        self.len.set(self.len.get() + 1);
    }

    fn on_remove_success(&self) {
        self.len.set(self.len.get() - 1);
    }

    fn on_clear(&self) {
        self.len.set(0);
    }

    fn invalidate(&self) {
        // Nothing to invalidate.
    }
}

impl<T: ?Sized, A: Allocator> WithLen<T, A> {
    pub(crate) fn new() -> Self {
        Self {
            len: Cell::new(0),
            _phantom: ::std::marker::PhantomData,
        }
    }
}

impl<T: ?Sized> WithLen<T, Global> {
    /// Creates a new empty list using this cache mode.
    pub fn new_list() -> OnceListCore<T, Global, WithLen<T, Global>> {
        OnceListCore {
            head_slot: NextSlot::new(),
            alloc: Global,
            cache_mode: WithLen::new(),
        }
    }
}

impl<T: ?Sized, A: Allocator> WithLen<T, A> {
    /// Creates a new empty list with the given allocator using this cache mode.
    pub fn new_list_in(alloc: A) -> OnceListCore<T, A, WithLen<T, A>> {
        OnceListCore {
            head_slot: NextSlot::new(),
            alloc,
            cache_mode: WithLen::new(),
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

    fn tail_slot_opt<'a>(&'a self) -> Option<&'a NextSlot<T, A>> {
        if let Some(p) = self.next_slot.get() {
            let slot = unsafe { p.as_ref() };
            if slot.get().is_none() {
                return Some(slot);
            }
        }
        None
    }

    fn on_push_success(&self, next_slot: &NextSlot<T, A>) {
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

impl<T: ?Sized> WithTailLen<T, Global> {
    /// Creates a new empty list using this cache mode.
    pub fn new_list() -> OnceListCore<T, Global, WithTailLen<T, Global>> {
        OnceListCore {
            head_slot: NextSlot::new(),
            alloc: Global,
            cache_mode: WithTailLen::new(),
        }
    }
}

impl<T: ?Sized, A: Allocator> WithTailLen<T, A> {
    /// Creates a new empty list with the given allocator using this cache mode.
    pub fn new_list_in(alloc: A) -> OnceListCore<T, A, WithTailLen<T, A>> {
        OnceListCore {
            head_slot: NextSlot::new(),
            alloc,
            cache_mode: WithTailLen::new(),
        }
    }
}
