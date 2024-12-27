// Copyright 2021 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![doc = include_str!("../README.md")]
#![cfg_attr(feature = "nightly", feature(allocator_api))]
#![cfg_attr(feature = "nightly", feature(box_into_inner))]
#![cfg_attr(feature = "nightly", feature(coerce_unsized))]
#![cfg_attr(feature = "nightly", feature(doc_cfg))]
#![cfg_attr(feature = "nightly", feature(once_cell_try_insert))]
#![cfg_attr(feature = "nightly", feature(ptr_metadata))]
#![cfg_attr(feature = "nightly", feature(unsize))]

#[cfg(feature = "nightly")]
use ::allocator_api2::alloc;
use ::allocator_api2::alloc::{Allocator, Global};
use ::allocator_api2::boxed::Box;
use ::std::any::Any;
use ::std::fmt::Debug;
#[cfg(feature = "nightly")]
use ::std::marker::Unsize;
use ::std::ops::DerefMut;

#[cfg(not(feature = "sync"))]
use ::std::cell::OnceCell;
#[cfg(feature = "sync")]
use ::std::sync::OnceLock as OnceCell;

/// A single linked list which behaves like [`std::cell::OnceCell`], but for multiple values.
///
/// # Usage
///
/// A simple example:
///
/// ```rust
/// use once_list2::OnceList;
///
/// // Create a new empty list. Note that the variable is immutable.
/// let list = OnceList::<i32>::new();
///
/// // You can push values to the list without the need for mutability.
/// list.push(1);
/// list.push(2);
///
/// // Or you can push multiple values at once.
/// list.extend([3, 4, 5]);
///
/// // You can iterate over the list.
/// assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
///
/// // Some methods are mutable only.
/// let mut list_mut = list;
///
/// // You can remove (take) a value from the list.
/// let removed = list_mut.remove(|&x| x % 2 == 0);
/// assert_eq!(removed, Some(2));
/// assert_eq!(list_mut.iter().copied().collect::<Vec<_>>(), vec![1, 3, 4, 5]);
/// ```
///
/// # (Almost nightly rustc only) Unsized types support
///
/// You can use the [unsized types] like `str`, `[u8]` or `dyn Display` as the value type of the `OnceList`.
///
/// Though you can use this feature without the nightly rustc compiler, you can not push or extend
/// the values to the list without the nightly compiler nor without the `nightly` feature enabled.
/// In that case you can only create an empty list, that's all.
///
/// In the nightly compiler and with the `nightly` feature
/// enabled, the additional methods like `push_unsized`, `remove_unsized_as` become available:
///
/// ```rust
/// # #[cfg(not(feature = "nightly"))]
/// # fn main() {}
/// # #[cfg(feature = "nightly")]
/// # fn main() {
/// // This code only works with the nightly compiler and the `nightly` feature enabled.
///
/// use once_list2::OnceList;
///
/// // Creating a `OnceList` for `[i32]`, the unsized type.
/// let list = OnceList::<[i32]>::new();
///
/// list.push_unsized([1] /* A sized array type, `[i32; 1]`, can be coerced into [i32].*/);
/// list.push_unsized([2, 3] /* Same for `[i32; 2] type. */);
///
/// // The normal methods like `iter` are available because it returns a reference to the value.
/// assert_eq!(list.iter().nth(0).unwrap(), &[1]);
/// assert_eq!(list.iter().nth(1).unwrap(), &[2, 3]);
///
/// let mut list_mut = list;
///
/// // `remove_unsized_as` method allows you to check the unsized value type and remove it.
/// let removed: Option<[i32; 2]> = unsafe {
///     list_mut.remove_unsized_as(|x| if x.len() == 2 {
///         Some(x.try_into().unwrap())
///     } else {
///         None
///     })
/// };
/// // The removed value is an array, not a slice!
/// assert_eq!(removed, Some([2, 3]));
/// # }
/// ```
/// [unsized types]: https://doc.rust-lang.org/book/ch19-04-advanced-types.html#dynamically-sized-types-and-the-sized-trait
///
#[derive(Clone)]
pub struct OnceList<T: ?Sized, A: Allocator = Global> {
    head: OnceCell<Box<Cons<T, T, A>, A>>,
    alloc: A,
}

/// A single linked list node.
///
/// Type parameter `T` and `U` are essentially the same type, but for rust's unsized coercion
/// feature, we need to separate them.
/// Then we can safely cast `&Cons<SizedT, U, A>` into `&Cons<UnsizedT, U, A>`.
#[derive(Clone)]
struct Cons<T: ?Sized, U: ?Sized, A: Allocator> {
    next: OnceCell<Box<Cons<U, U, A>, A>>,
    val: T,
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

    fn last_cell(&self) -> &OnceCell<Box<Cons<T, T, A>, A>> {
        let mut next_cell = &self.head;
        while let Some(next_box) = next_cell.get() {
            next_cell = &next_box.next;
        }
        next_cell
    }

    /// Returns an iterator over the `&T` references in the list.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let mut next_cell = &self.head;
        ::std::iter::from_fn(move || match next_cell.get() {
            Some(c) => {
                next_cell = &c.next;
                Some(&c.val)
            }
            None => None,
        })
    }

    /// Returns an iterator over the `&mut T` references in the list.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        struct Iter<'a, T: ?Sized, A: Allocator> {
            next_cell: &'a mut OnceCell<Box<Cons<T, T, A>, A>>,
        }
        impl<'a, T: ?Sized, A: Allocator> Iterator for Iter<'a, T, A> {
            type Item = &'a mut T;
            fn next(&mut self) -> Option<Self::Item> {
                let next_box = self.next_cell.get_mut()?;
                // Need to cast the `self` lifetime to `&'a` to update the `Self::Item`.
                let next_cons = unsafe { &mut *::std::ptr::from_mut(next_box.as_mut()) };
                self.next_cell = &mut next_cons.next;
                Some(&mut next_cons.val)
            }
        }
        Iter {
            next_cell: &mut self.head,
        }
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
    pub unsafe fn remove_unsized_as<U, P>(&mut self, mut pred: P) -> Option<U>
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
    fn remove_inner<P, F, U>(&mut self, mut pred: P, mut f: F) -> Option<U>
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
    fn push_inner<F, U>(&self, mut new_cons: Box<Cons<T, T, A>, A>, f: F) -> &U
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

impl<A: Allocator + Clone> OnceList<dyn Any, A> {
    /// Pushes an aribitrary value to the list, and returns the reference to that value.
    pub fn push_any<T: Any>(&self, val: T) -> &T {
        self.push_inner(
            Box::new_in(Cons::<T, dyn Any, A>::new(val), A::clone(&self.alloc)),
            // Safe because we know the given value is type `T`.
            |c| c.downcast_ref::<T>().unwrap(),
        )
    }

    /// Finds the first value in the list that is the same type as `T`, and returns the reference to that value.
    pub fn find_by_type<T: Any>(&self) -> Option<&T> {
        self.iter().find_map(|val| val.downcast_ref())
    }

    /// Removes the first value in the list that is the same type as `T`, and returns the value.
    pub fn remove_by_type<T: Any>(&mut self) -> Option<T> {
        self.remove_inner(
            |v| v.is::<T>(),
            |boxed_cons| {
                let cons_layout = alloc::Layout::for_value::<Cons<_, _, _>>(&boxed_cons);
                let (cons_ptr, alloc) = Box::into_non_null_with_allocator(boxed_cons);

                let Cons {
                    next: next_ref,
                    val: val_any_ref,
                } = unsafe { cons_ptr.as_ref() };
                // drop the `next` field.
                unsafe { ::std::ptr::read(next_ref) };

                let val_ref = <dyn Any>::downcast_ref::<T>(val_any_ref).unwrap();
                let val = unsafe { ::std::ptr::read(val_ref) };

                unsafe {
                    alloc.deallocate(cons_ptr.cast(), cons_layout);
                }

                val
            },
        )
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

pub struct IntoIter<T, A: Allocator>(OnceCell<Box<Cons<T, T, A>, A>>);

impl<T, A: Allocator> Iterator for IntoIter<T, A> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        let next_cell = self.0.take()?;
        let next_cons = Box::into_inner(next_cell);
        self.0 = next_cons.next;
        Some(next_cons.val)
    }
}

impl<T, A: Allocator + Clone> Extend<T> for OnceList<T, A> {
    /// Due to the definition of the `Extend` trait, this method takes `&mut self`.
    /// Use the [`OnceList::extend`] method instead if you want to use `&self`.
    fn extend<U: IntoIterator<Item = T>>(&mut self, iter: U) {
        <OnceList<T, A>>::extend(self, iter);
    }
}

impl<T, U: ?Sized, A: Allocator> Cons<T, U, A> {
    fn new(val: T) -> Self {
        Self {
            next: OnceCell::new(),
            val,
        }
    }
}

#[cfg(feature = "nightly")]
impl<T: ?Sized, A: Allocator> Cons<T, T, A> {
    fn new_boxed<U>(val: U, alloc: A) -> Box<Self, A>
    where
        U: Unsize<T>,
    {
        // As mentioned in the [`Cons`]'s document, this unsized coercion cast is safe!
        Box::<Cons<U, T, A>, A>::new_in(
            Cons::<U, T, A> {
                next: OnceCell::new(),
                val: val,
            },
            alloc,
        )
    }

    fn box_into_inner_box(self: Box<Self, A>) -> Box<T, A> {
        use ::std::ptr::{metadata, NonNull};

        let cons_layout = alloc::Layout::for_value::<Cons<T, T, A>>(&self);
        let layout = alloc::Layout::for_value::<T>(&self.val);
        let metadata = metadata(&self.val);
        let (raw_cons, alloc) = Box::into_raw_with_allocator(self);
        let dst = alloc.allocate(layout).unwrap();

        // Make sure to drop the `cons`'s unused fields.
        let Cons { next, val } = unsafe { &*raw_cons };
        let _ = unsafe { ::std::ptr::read(next) };
        let raw_src = val as *const T;

        // Do memcpy.
        unsafe {
            ::std::ptr::copy_nonoverlapping(
                raw_src.cast::<u8>(),
                dst.cast::<u8>().as_ptr(),
                layout.size(),
            );
        }

        // free the `cons`'s memory. Not `drop` because we already dropped the fields.
        unsafe {
            alloc.deallocate(NonNull::new(raw_cons).unwrap().cast(), cons_layout);
        }

        // Create a new fat pointer for dst by combining the thin pointer and the metadata.
        let dst = NonNull::<T>::from_raw_parts(dst.cast::<u8>(), metadata);

        unsafe { Box::from_non_null_in(dst, alloc) }
    }
}

/// A workaround for the missing `OnceCell::try_insert` method.
trait OnceCellExt<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)>;
}
#[cfg(not(feature = "nightly"))]
impl<T> OnceCellExt<T> for OnceCell<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)> {
        // The both unsafe blocks are safe because it's sure the cell value is set.
        match self.set(value) {
            Ok(()) => Ok(unsafe { self.get().unwrap_unchecked() }),
            Err(value) => Err((unsafe { self.get().unwrap_unchecked() }, value)),
        }
    }
}
#[cfg(feature = "nightly")]
impl<T> OnceCellExt<T> for OnceCell<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)> {
        self.try_insert(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let list = OnceList::<i32>::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.iter().next(), None);
    }

    #[test]
    fn test_default() {
        let list = OnceList::<i32>::default();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.iter().next(), None);
    }

    #[test]
    fn test_push() {
        let list = OnceList::new();
        let val = list.push(42);
        assert_eq!(val, &42);
        assert_eq!(list.len(), 1);
        assert_eq!(list.clone().into_iter().collect::<Vec<_>>(), vec![42]);

        list.push(100);
        list.push(3);
        assert_eq!(list.len(), 3);
        assert_eq!(list.into_iter().collect::<Vec<_>>(), vec![42, 100, 3]);
    }

    #[test]
    fn test_from_iter() {
        let list = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        assert_eq!(list.len(), 3);
        assert_eq!(list.into_iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn test_extend() {
        let list = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        list.extend([4, 5, 6]);
        assert_eq!(list.len(), 6);
        assert_eq!(list.into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_clear() {
        let mut list = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.iter().next(), None);
    }

    #[test]
    fn test_first_last() {
        let empty_list = OnceList::<i32>::new();
        assert_eq!(empty_list.first(), None);
        assert_eq!(empty_list.last(), None);

        let single_list = [42].into_iter().collect::<OnceList<_>>();
        assert_eq!(single_list.first(), Some(&42));
        assert_eq!(single_list.last(), Some(&42));

        let multiple_list = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        assert_eq!(multiple_list.first(), Some(&1));
        assert_eq!(multiple_list.last(), Some(&3));
    }

    #[test]
    fn test_contains() {
        let list = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        assert!(list.contains(&1));
        assert!(list.contains(&2));
        assert!(list.contains(&3));
        assert!(!list.contains(&0));
        assert!(!list.contains(&4));

        let empty_list = OnceList::<i32>::new();
        assert!(!empty_list.contains(&1));
    }

    #[test]
    fn test_remove() {
        let mut list = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        assert_eq!(list.remove(|&v| v == 2), Some(2));
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![&1, &3]);

        assert_eq!(list.remove(|&v| v == 0), None);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![&1, &3]);

        assert_eq!(list.remove(|&v| v == 1), Some(1));
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![&3]);

        assert_eq!(list.remove(|&v| v == 3), Some(3));
        assert!(list.is_empty());
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_unsized_slice_push() {
        let list: OnceList<[i32]> = OnceList::new();
        let first = list.push_unsized([1]);
        let second = list.push_unsized([2, 3]);
        assert_eq!(first, &[1]);
        assert_eq!(second, &[2, 3]);

        assert_eq!(list.iter().nth(0), Some(&[1] as &[i32]));
        assert_eq!(list.iter().nth(1), Some(&[2, 3] as &[i32]));
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_unsized_dyn_push() {
        let list: OnceList<dyn ToString> = OnceList::new();
        let first = list.push_unsized(1);
        let second = list.push_unsized("hello");
        assert_eq!(first.to_string(), "1");
        assert_eq!(second.to_string(), "hello");

        assert_eq!(
            list.iter().nth(0).map(<dyn ToString>::to_string),
            Some("1".to_string())
        );
        assert_eq!(
            list.iter().nth(1).map(<dyn ToString>::to_string),
            Some("hello".to_string())
        );
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_unsized_slice_remove_into_box() {
        let list = OnceList::<[i32]>::new();
        list.push_unsized([1]);
        list.push_unsized([2, 3]);
        list.push_unsized([4, 5, 6]);

        let mut list = list;
        let removed = list.remove_into_box(|s| s.len() == 2);
        assert_eq!(removed, Some(Box::new([2, 3]) as Box<[i32]>));
        assert_eq!(list.len(), 2);
        assert_eq!(list.iter().nth(0), Some(&[1] as &[i32]));
        assert_eq!(list.iter().nth(1), Some(&[4, 5, 6] as &[i32]));
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_unsized_dyn_remove_into_box() {
        let list = OnceList::<dyn ToString>::new();
        list.push_unsized(1);
        list.push_unsized("hello");
        list.push_unsized(42);

        let mut list = list;
        let removed = list.remove_into_box(|s| s.to_string() == "hello");
        assert_eq!(removed.map(|s| s.to_string()), Some("hello".to_string()));
        assert_eq!(list.len(), 2);
        assert_eq!(
            list.iter().nth(0).map(|s| s.to_string()),
            Some("1".to_string())
        );
        assert_eq!(
            list.iter().nth(1).map(|s| s.to_string()),
            Some("42".to_string())
        );
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_unsized_slice_remove_as() {
        let list = OnceList::<[i32]>::new();
        list.push_unsized([1]);
        list.push_unsized([2, 3]);
        list.push_unsized([4, 5, 6]);

        let mut list = list;
        let removed: Option<[i32; 2]> = unsafe { list.remove_unsized_as(|s| s.try_into().ok()) };
        assert_eq!(removed, Some([2, 3]));
        assert_eq!(list.len(), 2);
        assert_eq!(list.iter().nth(0), Some(&[1] as &[i32]));
        assert_eq!(list.iter().nth(1), Some(&[4, 5, 6] as &[i32]));
    }
}
