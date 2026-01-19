use ::allocator_api2::alloc::Allocator;
#[cfg(feature = "nightly")]
use ::allocator_api2::boxed::Box;
#[cfg(feature = "nightly")]
use ::std::marker::Unsize;

use crate::tail_mode::TailSlot;

/// A single linked list node.
///
/// ## Why are there two type parameters (`T` and `U`)?
///
/// - `T` is the value type stored in **this** node (`val: T`).
/// - `U` is the value type stored in the **next and subsequent** nodes
///   (`next: OnceCell<Box<Cons<U, U, A>, A>>`).
///
/// In the common (sized) case, `T` and `U` are the same type and you can read this as a normal
/// homogeneous linked list.
///
/// For the *unsized* use case (e.g. `str`, `[u8]`, `dyn Trait`), Rust's unsized coercion works only
/// on the **current** value type (`T`). The rest of the list must keep a single, fixed node layout.
/// In other words, although the name is `Cons<T, U, A>`, the \"list item type\" for nodes after the
/// first one is effectively fixed as `U` by the linked-list design (`Cons<U, U, A>` repeats).
///
/// Separating `T` and `U` lets us safely treat:
/// - `&Cons<SizedT, U, A>` as `&Cons<UnsizedT, U, A>` (coercing only the current `val`),
/// while keeping the tail (`next`) layout unchanged.
#[derive(Clone)]
pub(crate) struct Cons<T: ?Sized, U: ?Sized, A: Allocator> {
    pub(crate) next: TailSlot<U, A>,
    pub(crate) val: T,
}

impl<T, U: ?Sized, A: Allocator> Cons<T, U, A> {
    pub(crate) fn new(val: T) -> Self {
        Self {
            next: TailSlot::new(),
            val,
        }
    }
}

#[cfg(feature = "nightly")]
impl<T: ?Sized, A: Allocator> Cons<T, T, A> {
    pub(crate) fn new_boxed<U>(val: U, alloc: A) -> Box<Self, A>
    where
        U: Unsize<T>,
    {
        // As mentioned in the [`Cons`]'s document, this unsized coercion cast is safe!
        Box::<Cons<U, T, A>, A>::new_in(
            Cons::<U, T, A> {
                next: TailSlot::new(),
                val,
            },
            alloc,
        )
    }

    pub(crate) fn box_into_inner_box(self: Box<Self, A>) -> Box<T, A> {
        use ::allocator_api2::alloc;
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

        // Free the `cons`'s memory. Not `drop` because we already dropped the fields.
        unsafe {
            alloc.deallocate(NonNull::new(raw_cons).unwrap().cast(), cons_layout);
        }

        // Create a new fat pointer for dst by combining the thin pointer and the metadata.
        let dst = NonNull::<T>::from_raw_parts(dst.cast::<u8>(), metadata);

        unsafe { Box::from_non_null_in(dst, alloc) }
    }
}

