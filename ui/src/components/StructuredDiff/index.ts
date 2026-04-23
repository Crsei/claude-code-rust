/**
 * Barrel for the Lite-native structured diff helpers.
 *
 * Exports a pure hunk parser (`parseUnifiedDiff`), a simple
 * `old_string` / `new_string` hunk builder (`hunkFromEdit`), and both
 * renderers — `<StructuredDiff>` for a single hunk and
 * `<StructuredDiffList>` for an array of hunks with ellipsis separators
 * between them. Split matches the upstream layout in
 * `ui/examples/upstream-patterns/src/components/StructuredDiff.tsx` +
 * `StructuredDiffList.tsx`.
 *
 * Consumers are the new `permissions/` dialog family and the richer
 * file-edit tool-result rendering — both added for Issue 04.
 */
export {
  hunkFromEdit,
  hunkGutterWidth,
  parseUnifiedDiff,
  type DiffHunk,
  type DiffLine,
  type DiffLineKind,
} from './hunks.js'
export { StructuredDiff } from '../StructuredDiff.js'
export { StructuredDiffList } from '../StructuredDiffList.js'
