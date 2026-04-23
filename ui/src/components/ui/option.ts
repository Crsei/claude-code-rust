/**
 * Shared option types used across selection widgets (`Select`, `TreeSelect`,
 * etc.). Ported from
 * `ui/examples/upstream-patterns/src/components/ui/option.ts` — upstream
 * ships this as an auto-generated stub, so we seed it with the concrete
 * shapes every caller in this repo already uses.
 */

/** Base option — enough for a plain `<Select>`. */
export interface Option<V extends string | number = string> {
  label: string
  value: V
  /** Optional inline description (rendered dim next to the label). */
  description?: string
  /** When true, the option is un-selectable (rendered dim with no cursor stop). */
  disabled?: boolean
}

/** `Option` plus the optional `dimDescription` flag that `TreeSelect` uses. */
export interface OptionWithDescription<V extends string | number = string>
  extends Option<V> {
  /** When true, force the description to render dimmed even on the selected
   *  row (matches upstream `dimDescription` behaviour). */
  dimDescription?: boolean
}
