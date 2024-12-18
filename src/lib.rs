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

use ::allocator_api2::alloc::{Allocator, Global};
use ::allocator_api2::boxed::Box;
use ::std::fmt::Debug;
use ::std::ops::DerefMut;

#[cfg(not(feature = "sync"))]
use ::std::cell::OnceCell;
#[cfg(feature = "sync")]
use ::std::sync::OnceLock as OnceCell;

/// A single linked list which behaves like [`std::cell::OnceCell`], but for multiple values.
/// See the crate document for the examples.
#[derive(Clone)]
pub struct OnceList<T, A: Allocator = Global> {
    head: OnceCell<Box<Cons<T, A>, A>>,
    alloc: A,
}

#[derive(Clone)]
struct Cons<T, A: Allocator> {
    next: OnceCell<Box<Cons<T, A>, A>>,
    val: T,
}

impl<T> OnceList<T, Global> {
    /// Creates a new empty `OnceList`. This method does not allocate.
    pub fn new() -> Self {
        Self {
            head: OnceCell::new(),
            alloc: Global,
        }
    }
}
impl<T, A: Allocator> OnceList<T, A> {
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

    fn last_cell(&self) -> &OnceCell<Box<Cons<T, A>, A>> {
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
        struct Iter<'a, T, A: Allocator> {
            next_cell: &'a mut OnceCell<Box<Cons<T, A>, A>>,
        }
        impl<'a, T, A: Allocator> Iterator for Iter<'a, T, A> {
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

    /// Find a first value in the list matches the predicate, remove that item from the list,
    /// and then returns that value.
    pub fn remove<P>(&mut self, mut pred: P) -> Option<T>
    where
        P: FnMut(&T) -> bool,
    {
        let mut next_cell = &mut self.head;
        while let Some(next_ref) = next_cell.get() {
            if pred(&next_ref.val) {
                // Safe because we are sure the `next_cell` value is set.
                let next_box = next_cell.take().unwrap();
                let mut next_cons = Box::into_inner(next_box);

                // reconnect the list
                if let Some(next_next) = next_cons.next.take() {
                    let _ = next_cell.set(next_next);
                }

                return Some(next_cons.val);
            }
            // Safe because we are sure the `next_cell` value is set.
            next_cell = &mut next_cell.get_mut().unwrap().next;
        }
        None
    }
}

impl<T: Sized, A: Allocator + Clone> OnceList<T, A> {
    /// Appends a value to the list, and returns the reference to that value.
    ///
    /// Note that this method takes `&self`, not `&mut self`.
    pub fn push(&self, val: T) -> &T {
        let mut next_cell = &self.head;
        let mut boxed_cons = Box::new_in(Cons::new(val), self.alloc.clone());
        loop {
            match next_cell.set(boxed_cons) {
                Ok(()) => {
                    // Safe because we are sure the `next` value is set.
                    return &unsafe { next_cell.get().unwrap_unchecked() }.val;
                }
                Err(new_val_cons) => {
                    // Safe because we are sure the `next` value is set.
                    next_cell = &unsafe { next_cell.get().unwrap_unchecked() }.next;
                    boxed_cons = new_val_cons;
                }
            }
        }
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

impl<T> Default for OnceList<T, Global> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug, A: Allocator> Debug for OnceList<T, A> {
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

pub struct IntoIter<T, A: Allocator>(OnceCell<Box<Cons<T, A>, A>>);

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

impl<T, A: Allocator> Cons<T, A> {
    fn new(val: T) -> Self {
        Self {
            next: OnceCell::new(),
            val,
        }
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
}
