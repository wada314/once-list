# AI References (once-list2)

## 2026-01-20

- Renamed `OnceListCore` internals for clarity:
  - `mode: M` -> `cache_mode: C`
  - `head: TailSlot<...>` -> `head_slot: TailSlot<...>`
- Clarified `TailSlot` documentation: it is conceptually a "next slot" used for the head slot, node next slots, and optional tail insertion caching.

