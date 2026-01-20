# AI References (once-list2)

## 2026-01-20

- Renamed `OnceListCore` internals for clarity:
  - `mode: M` -> `cache_mode: C`
  - `head: TailSlot<...>` -> `head_slot: TailSlot<...>`
- Clarified slot naming and semantics:
  - Renamed `TailSlot` -> `NextSlot` (the slot is conceptually "next", even when used as the list's head slot)
  - Updated docs to explain its roles: head slot, per-node next slot, and optional tail insertion caching.
- Renamed cache invalidation hook for naming consistency:
  - `CacheMode::invalidate()` -> `CacheMode::on_structure_change()`
- Made CacheMode hook defaults consistent:
  - `CacheMode::on_push_success()` now has a default no-op implementation (like `on_remove_success` / `on_clear`)
- Removed redundant list constructors from cache mode types:
  - Dropped `WithTail::{new_list,new_list_in}`, `WithLen::{new_list,new_list_in}`, `WithTailLen::{new_list,new_list_in}`
  - Docs now point to `OnceListWith*::{new,new_in}` instead.
- Fixed feature-flag build:
  - Enabling `nightly` failed due to missing `OnceCell` import in `remove_unsized_as`; added `#[cfg(feature="nightly")] use crate::OnceCell;` in `src/once_list.rs`.
- Documented `sync` + cache-mode thread-safety:
  - Clarified in `readme.md` that `sync` swaps `OnceCell` to `OnceLock`, but cache modes are still single-thread oriented and not `Sync`.
- Fixed `extend()` to honor caching + avoid value loss under contention:
  - `OnceListCore::extend(&self, ..)` now starts from `CacheMode::tail_slot_opt()` when available, otherwise from the head.
  - Uses `try_insert2` retry loop (same approach as `push_inner`) so `--features sync` doesn't drop elements when concurrent inserts happen.
- Added per-file copyright/license headers:
  - Inserted the same Apache-2.0 header used in `src/lib.rs` at the top of every Rust source file under `src/`.

