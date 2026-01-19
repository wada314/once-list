use ::allocator_api2::alloc;
use ::allocator_api2::alloc::{Allocator, Global};
use ::allocator_api2::boxed::Box;
use ::std::fmt::Debug;
use ::std::hash::Hash;
#[cfg(feature = "nightly")]
use ::std::marker::Unsize;
use ::std::ops::DerefMut;

use crate::cons::Cons;
use crate::iter::{IntoIter, Iter, IterMut};
use crate::oncecell_ext::OnceCellExt;
use crate::OnceCell;

/// A single linked list which behaves like [`std::cell::OnceCell`], but for multiple values.
#[derive(Clone)]
pub struct OnceList<T: ?Sized, A: Allocator = Global> {
    pub(crate) head: OnceCell<Box<Cons<T, T, A>, A>>,
    pub(crate) alloc: A,
}

impl<T: ?Sized> OnceList<T, Global> {
    /// Creates a new empty `OnceList`. This method does not allocate.
    pub fn new() -> Self {
        Self {
            head: OnceCell::new(),
            alloc: Global,
        }
    }
}

impl<T: ?Sized, A: Allocator> OnceList<T, A> {
    /// Creates a new empty `OnceList` with the given allocator. This method does not allocate.
    pub fn new_in(alloc: A) -> Self {
        Self {
            head: OnceCell::new(),
            alloc,
        }
    }

    /// Returns the number of values in the list. This method is O(n).
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.head.get().is_none()
    }

    /// Returns `true` if the list contains the value.
    pub fn contains(&self, val: &T) -> bool
    where
        T: PartialEq,
    {
        self.iter().any(|v| v == val)
    }

    /// Returns a first value, if it exists.
    pub fn first(&self) -> Option<&T> {
        self.head.get().map(|c| &c.val)
    }

    /// Returns a mutable reference to the first value, if it exists.
    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.head.get_mut().map(|c| &mut c.val)
    }

    /// Returns a last value, if it exists.
    /// This method is O(n).
    pub fn last(&self) -> Option<&T> {
        let mut last_opt = None;
        let mut next_cell = &self.head;
        while let Some(next_box) = next_cell.get() {
            last_opt = Some(&next_box.val);
            next_cell = &next_box.next;
        }
        last_opt
    }

    /// Returns a mutable reference to the last value, if it exists.
    /// This method is O(n).
    pub fn last_mut(&mut self) -> Option<&mut T> {
        let mut last_opt = None;
        let mut next_cell = &mut self.head;
        while let Some(next_box) = next_cell.get_mut() {
            let next_cons = Box::deref_mut(next_box);
            last_opt = Some(&mut next_cons.val);
            next_cell = &mut next_cons.next;
        }
        last_opt
    }

    pub(crate) fn last_cell(&self) -> &OnceCell<Box<Cons<T, T, A>, A>> {
        let mut next_cell = &self.head;
        while let Some(next_box) = next_cell.get() {
            next_cell = &next_box.next;
        }
        next_cell
    }

    /// Returns an iterator over the `&T` references in the list.
    pub fn iter(&self) -> Iter<'_, T, A> {
        Iter::new(&self.head)
    }

    /// Returns an iterator over the `&mut T` references in the list.
    pub fn iter_mut(&mut self) -> IterMut<'_, T, A> {
        IterMut::new(&mut self.head)
    }

    /// Clears the list, dropping all values.
    pub fn clear(&mut self) {
        self.head = OnceCell::new();
    }

    /// Returns an allocator of this struct.
    pub fn allocator(&self) -> &A {
        &self.alloc
    }
}

impl<T: ?Sized, A: Allocator> OnceList<T, A> {
    /// Removes the first value in the list that matches the predicate, and returns the value as a boxed value.
    ///
    /// This method supports the unsized value type `T` as well.
    ///
    /// Note that even though this method returns a boxed value, the box is something re-allcoated.
    /// So this method might not be efficient as you expect.
    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "nightly")))]
    pub fn remove_into_box<P>(&mut self, pred: P) -> Option<Box<T, A>>
    where
        P: FnMut(&T) -> bool,
    {
        self.remove_inner(pred, |boxed_cons| boxed_cons.box_into_inner_box())
    }

    /// Removes the first value in the list that matches the predicate, and returns the value.
    ///
    /// The predicate function `pred` should return `Some(&U)` if the value is found,
    /// and the returned reference `&U` must be the same address as the value given in the `pred`.
    ///
    /// # Safety
    /// This method is unsafe because it requires the predicate to return a reference to the same address as the value.
    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "nightly")))]
    pub unsafe fn remove_unsized_as<P, U>(&mut self, mut pred: P) -> Option<U>
    where
        P: FnMut(&T) -> Option<&U>,
    {
        let found_sized_ptr: OnceCell<*const U> = OnceCell::new();
        self.remove_inner(
            |val| {
                if let Some(val) = pred(val) {
                    // We only set the value once, so this is safe.
                    found_sized_ptr.set(val as *const U).unwrap();
                    true
                } else {
                    false
                }
            },
            |boxed_cons| -> U {
                // Given the boxed cons with the unsized value type `T`,
                // and returns the sized type value `U` by value (i.e. out of the box).

                // We are sure the `found_sized_ptr` is set.
                let found_sized_ptr: *const U = *found_sized_ptr.get().unwrap();

                let cons_layout = alloc::Layout::for_value::<Cons<T, T, A>>(&boxed_cons);
                let (cons_ptr, alloc) = Box::into_non_null_with_allocator(boxed_cons);
                let val_ptr = &unsafe { cons_ptr.as_ref() }.val as *const T;

                // Double check the ptr returned by the `pred` is the same as the pointer we extracted from the cons.
                debug_assert_eq!(val_ptr as *const U, found_sized_ptr);

                // Load (memcpy) the value into the output variable.
                let result = unsafe { ::std::ptr::read(val_ptr as *const U) };

                // Free the cons memory.
                unsafe { alloc.deallocate(cons_ptr.cast(), cons_layout) };

                result
            },
        )
    }

    /// An inner implementeation for `remove_xxx` methods.
    pub(crate) fn remove_inner<P, F, U>(&mut self, mut pred: P, mut f: F) -> Option<U>
    where
        P: FnMut(&T) -> bool,
        F: FnMut(Box<Cons<T, T, A>, A>) -> U,
    {
        let mut next_cell = &mut self.head;
        while let Some(next_ref) = next_cell.get() {
            if pred(&next_ref.val) {
                // Safe because we are sure the `next_cell` value is set.
                let mut next_box = next_cell.take().unwrap();

                // reconnect the list
                if let Some(next_next) = next_box.next.take() {
                    let _ = next_cell.set(next_next);
                }

                return Some(f(next_box));
            }
            // Safe because we are sure the `next_cell` value is set.
            next_cell = &mut next_cell.get_mut().unwrap().next;
        }
        None
    }
}

impl<T: ?Sized, A: Allocator + Clone> OnceList<T, A> {
    /// An unsized version of the [`OnceList::push`] method.
    ///
    /// You can push a sized value to the list. For exaple, you can push `[i32; 3]` to the list of `[i32]`.
    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "nightly")))]
    pub fn push_unsized<U: Unsize<T>>(&self, val: U) -> &U {
        let boxed_cons = Cons::new_boxed(val, self.alloc.clone());
        self.push_inner(boxed_cons, |c| unsafe { &*(c as *const T as *const U) })
    }

    /// An inner implementation for the `push_xxx` methods.
    pub(crate) fn push_inner<F, U>(&self, mut new_cons: Box<Cons<T, T, A>, A>, f: F) -> &U
    where
        F: FnOnce(&T) -> &U,
    {
        let mut next_cell = &self.head;
        loop {
            match next_cell.try_insert2(new_cons) {
                Ok(new_cons) => {
                    return f(&new_cons.val);
                }
                Err((cur_cons, new_cons2)) => {
                    next_cell = &cur_cons.next;
                    new_cons = new_cons2;
                }
            }
        }
    }
}

impl<T, A: Allocator> OnceList<T, A> {
    /// Find a first value in the list matches the predicate, remove that item from the list,
    /// and then returns that value.
    pub fn remove<P>(&mut self, mut pred: P) -> Option<T>
    where
        P: FnMut(&T) -> bool,
    {
        self.remove_inner(&mut pred, |boxed_cons| Box::into_inner(boxed_cons).val)
    }
}

impl<T, A: Allocator + Clone> OnceList<T, A> {
    /// Appends a value to the list, and returns the reference to that value.
    ///
    /// Note that this method takes `&self`, not `&mut self`.
    pub fn push(&self, val: T) -> &T {
        let boxed_cons = Box::new_in(Cons::new(val), self.alloc.clone());
        self.push_inner(boxed_cons, |c| c)
    }

    /// An almost same method with the [`std::iter::Extend::extend`],
    /// though this method takes `&self` instead of `&mut self`.
    ///
    /// [`std::iter::Extend::extend`]: https://doc.rust-lang.org/std/iter/trait.Extend.html#tymethod.extend
    pub fn extend<U: IntoIterator<Item = T>>(&self, iter: U) {
        let mut last_cell = self.last_cell();
        let alloc = self.allocator();
        for val in iter {
            let _ = last_cell.set(Box::new_in(Cons::new(val), A::clone(alloc)));
            last_cell = &unsafe { &last_cell.get().unwrap_unchecked() }.next;
        }
    }
}

impl<T: ?Sized> Default for OnceList<T, Global> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized + Debug, A: Allocator> Debug for OnceList<T, A> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: ?Sized + PartialEq, A: Allocator> PartialEq for OnceList<T, A> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: ?Sized + Eq, A: Allocator> Eq for OnceList<T, A> {}

impl<T: ?Sized + Hash, A: Allocator> Hash for OnceList<T, A> {
    fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.len());
        for val in self.iter() {
            val.hash(state);
        }
    }
}

impl<T> FromIterator<T> for OnceList<T, Global> {
    fn from_iter<U: IntoIterator<Item = T>>(iter: U) -> Self {
        let list = Self::new();
        let mut last_cell = &list.head;
        for val in iter {
            let _ = last_cell.set(Box::new(Cons::new(val)));
            last_cell = &unsafe { &last_cell.get().unwrap_unchecked() }.next;
        }
        list
    }
}

impl<T, A: Allocator> IntoIterator for OnceList<T, A> {
    type Item = T;
    type IntoIter = IntoIter<T, A>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.head)
    }
}

impl<T, A: Allocator + Clone> Extend<T> for OnceList<T, A> {
    /// Due to the definition of the `Extend` trait, this method takes `&mut self`.
    /// Use the [`OnceList::extend`] method instead if you want to use `&self`.
    fn extend<U: IntoIterator<Item = T>>(&mut self, iter: U) {
        <OnceList<T, A>>::extend(self, iter);
    }
}

