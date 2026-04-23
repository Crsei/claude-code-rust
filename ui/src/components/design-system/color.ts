import { c } from '../../theme.js'

/**
 * Lite-native port of the upstream
 * `ui/examples/upstream-patterns/src/components/design-system/color.ts`.
 *
 * Upstream's `color()` returned a curried `(text) => string` that wrapped
 * Ink's `colorize` with ANSI escapes. OpenTUI's renderer consumes a
 * plain hex/ansi string directly via the `fg`/`bg` JSX props, so this
 * helper just resolves a theme key or passthrough literal into a
 * renderable color string.
 */

type ThemeKey = keyof typeof c
type RawColor = string

export type ColorLike = ThemeKey | RawColor | undefined

function isRawColor(v: string): boolean {
  return (
    v.startsWith('rgb(') ||
    v.startsWith('#') ||
    v.startsWith('ansi256(') ||
    v.startsWith('ansi:')
  )
}

/**
 * Resolves a theme-key / raw-color input to the raw color string our
 * JSX props accept. Returns `undefined` when the input is empty so
 * `<text fg={resolve(x)}>` falls through to the rendered default.
 */
export function resolveColor(input: ColorLike): string | undefined {
  if (!input) return undefined
  if (isRawColor(input)) return input
  const key = input as ThemeKey
  return (c as Record<string, string>)[key]
}
