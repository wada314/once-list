# AI References (once-list2)

## 2026-01-20

- Renamed `OnceListCore` internals for clarity:
  - `mode: M` -> `cache_mode: C`
  - `head: TailSlot<...>` -> `head_slot: TailSlot<...>`
- Clarified slot naming and semantics:
  - Renamed `TailSlot` -> `NextSlot` (the slot is conceptually "next", even when used as the list's head slot)
  - Updated docs to explain its roles: head slot, per-node next slot, and optional tail insertion caching.

