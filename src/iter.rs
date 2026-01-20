use ::allocator_api2::alloc::Allocator;
use ::allocator_api2::alloc::Global;
use ::allocator_api2::boxed::Box;

use crate::cache_mode::TailSlot;

/// An iterator over references in a [`crate::OnceList`].
///
/// This iterator is intentionally a named type (instead of `impl Iterator`) so that downstream
/// crates can store it in structs without boxing or dynamic dispatch.
///
/// **Important**: This iterator observes newly pushed elements. If you reach the end (i.e. `next()`
/// returns `None`) and later call `OnceList::push()`, calling `next()` again on the same `Iter`
/// can yield the newly pushed element.
#[derive(Clone, Copy)]
pub struct Iter<'a, T: ?Sized, A: Allocator = Global> {
    pub(crate) next_slot: &'a TailSlot<T, A>,
}

impl<'a, T: ?Sized + 'a, A: Allocator> Iterator for Iter<'a, T, A> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let next_box = self.next_slot.get()?;
        self.next_slot = &next_box.next;
        Some(&next_box.val)
    }
}

/// A mutable iterator over references in a [`crate::OnceList`].
///
/// This iterator is intentionally a named type (instead of `impl Iterator`) so that downstream
/// crates can store it in structs without boxing or dynamic dispatch.
///
/// Note: Due to the singly-linked structure and internal `OnceCell`, advancing the iterator needs
/// to update the internal pointer. To return `&'a mut T`, this iterator uses a small amount of
/// `unsafe` internally (mirroring the previous inlined implementation of `iter_mut()`).
pub struct IterMut<'a, T: ?Sized, A: Allocator = Global> {
    pub(crate) next_slot: &'a mut TailSlot<T, A>,
}

impl<'a, T: ?Sized + 'a, A: Allocator> Iterator for IterMut<'a, T, A> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let next_box = self.next_slot.get_mut()?;
        // Need to cast the `self` lifetime to `&'a` to update the `Self::Item`.
        let next_cons = unsafe { &mut *::std::ptr::from_mut(next_box.as_mut()) };
        self.next_slot = &mut next_cons.next;
        Some(&mut next_cons.val)
    }
}

pub struct IntoIter<T, A: Allocator>(pub(crate) TailSlot<T, A>);

impl<T, A: Allocator> Iterator for IntoIter<T, A> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let next_cell = self.0.take()?;
        let next_cons = Box::into_inner(next_cell);
        self.0 = next_cons.next;
        Some(next_cons.val)
    }
}

impl<'a, T: ?Sized, A: Allocator> Iter<'a, T, A> {
    pub(crate) fn new(next_slot: &'a TailSlot<T, A>) -> Self {
        Self { next_slot }
    }
}

impl<'a, T: ?Sized, A: Allocator> IterMut<'a, T, A> {
    pub(crate) fn new(next_slot: &'a mut TailSlot<T, A>) -> Self {
        Self { next_slot }
    }
}
