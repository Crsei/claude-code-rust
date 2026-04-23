import { SyntaxStyle } from '@opentui/core'

/**
 * Shared syntax style for OpenTUI markdown/code primitives.
 *
 * OpenTUI requires a SyntaxStyle instance even when callers only need
 * plain rendering. Keeping one app-lifetime instance avoids allocating a
 * native style table for every message row.
 */
export const defaultSyntaxStyle = SyntaxStyle.create()
