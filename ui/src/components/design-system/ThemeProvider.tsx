import React, { createContext, useContext, type ReactNode } from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `ThemeProvider`. Exposes the same
 * hex-palette `c` object as the `useTheme()` hook upstream returns —
 * callers can swap palettes at runtime by wrapping a subtree.
 */

export type Theme = typeof c

const ThemeContext = createContext<Theme>(c)

type Props = {
  theme?: Partial<Theme>
  children: ReactNode
}

export function ThemeProvider({ theme, children }: Props) {
  const merged: Theme = theme ? ({ ...c, ...theme } as Theme) : c
  return (
    <ThemeContext.Provider value={merged}>{children}</ThemeContext.Provider>
  )
}

export function useTheme(): Theme {
  return useContext(ThemeContext)
}
