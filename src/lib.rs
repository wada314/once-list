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

#[cfg(not(feature = "sync"))]
pub(crate) use ::std::cell::OnceCell;
#[cfg(feature = "sync")]
pub(crate) use ::std::sync::OnceLock as OnceCell;

mod any;
mod cache_mode;
mod cons;
mod iter;
mod once_list;
mod oncecell_ext;

pub use crate::cache_mode::{NoCache, WithLen, WithTail, WithTailLen};
pub use crate::iter::{IntoIter, Iter, IterMut};
pub use crate::once_list::OnceList;
pub use crate::once_list::OnceListWithLen;
pub use crate::once_list::OnceListWithTail;
pub use crate::once_list::OnceListWithTailLen;

#[cfg(test)]
mod tests {
    use super::*;
    use ::std::hash::Hash;

    #[test]
    fn test_iter_sees_push_after_exhausted() {
        let list = OnceList::<i32>::new();
        list.push(1);

        let mut it = list.iter();
        assert_eq!(it.next(), Some(&1));
        assert_eq!(it.next(), None);

        // After the iterator reached the end, pushing a new element should make it visible
        // from the same iterator.
        list.push(2);
        assert_eq!(it.next(), Some(&2));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_iter_sees_extend_after_exhausted() {
        let list = OnceList::<i32>::new();
        list.push(1);

        let mut it = list.iter();
        assert_eq!(it.next(), Some(&1));
        assert_eq!(it.next(), None);

        // Same property should hold for `extend()` as well.
        list.extend([2, 3]);
        assert_eq!(it.next(), Some(&2));
        assert_eq!(it.next(), Some(&3));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_iter_mut_allows_in_place_update() {
        let mut list = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        for v in list.iter_mut() {
            *v += 10;
        }
        assert_eq!(list.into_iter().collect::<Vec<_>>(), vec![11, 12, 13]);
    }

    #[test]
    fn test_iter_mut_empty_and_singleton() {
        // Empty list
        let mut empty = OnceList::<i32>::new();
        let mut it = empty.iter_mut();
        assert!(it.next().is_none());

        // Singleton list
        let mut single = OnceList::<i32>::new();
        single.push(1);
        let mut it = single.iter_mut();
        let v = it.next().unwrap();
        *v = 2;
        assert!(it.next().is_none());
        assert_eq!(single.into_iter().collect::<Vec<_>>(), vec![2]);
    }

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
    fn test_eq() {
        let list1 = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        let list2 = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        assert_eq!(list1, list2);

        let list3 = [1, 2, 4].into_iter().collect::<OnceList<_>>();
        assert_ne!(list1, list3);

        let list4 = OnceList::<i32>::new();
        assert_eq!(list4, list4);
        assert_ne!(list1, list4);
    }

    #[test]
    fn test_hash() {
        use ::std::hash::{DefaultHasher, Hasher};
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        let list1 = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        let list2 = [1, 2, 3].into_iter().collect::<OnceList<_>>();
        list1.hash(&mut hasher1);
        list2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());

        let list3 = [1, 2, 4].into_iter().collect::<OnceList<_>>();
        let mut hasher3 = DefaultHasher::new();
        list3.hash(&mut hasher3);
        assert_ne!(hasher1.finish(), hasher3.finish());

        // make sure the hasher is prefix-free.
        // See https://doc.rust-lang.org/beta/std/hash/trait.Hash.html#prefix-collisions
        let tuple1 = (
            [1, 2].into_iter().collect::<OnceList<_>>(),
            [3].into_iter().collect::<OnceList<_>>(),
        );
        let tuple2 = (
            [1].into_iter().collect::<OnceList<_>>(),
            [2, 3].into_iter().collect::<OnceList<_>>(),
        );
        let mut hasher4 = DefaultHasher::new();
        let mut hasher5 = DefaultHasher::new();
        tuple1.hash(&mut hasher4);
        tuple2.hash(&mut hasher5);
        assert_ne!(hasher4.finish(), hasher5.finish());
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
