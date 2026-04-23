/**
 * Compatibility shim. The `OrderedList` / `OrderedListItem` implementation
 * moved into `./ui/` (matching the upstream layout in
 * `ui/examples/upstream-patterns/src/components/ui/`). This flat file
 * re-exports the same symbols so existing callers like [Onboarding.tsx](./Onboarding.tsx)
 * keep resolving without any path changes.
 *
 * New call sites should import from `./ui/` directly.
 */
export { OrderedList, OrderedListItem } from './ui/OrderedList.js'
