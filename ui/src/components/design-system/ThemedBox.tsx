import React, { type ReactNode } from 'react'
import { useTheme, type Theme } from './ThemeProvider.js'
import { resolveColor, type ColorLike } from './color.js'

/**
 * Lite-native port of upstream's `ThemedBox`. Thin wrapper around the
 * OpenTUI `<box>` primitive that accepts theme-key or raw-color strings
 * for `backgroundColor` / `borderColor`.
 */

type Props = {
  children?: ReactNode
  backgroundColor?: ColorLike
  borderColor?: ColorLike
  borderStyle?: 'single' | 'double' | 'rounded' | 'heavy'
  padding?: number
  paddingX?: number
  paddingY?: number
  margin?: number
  marginX?: number
  marginY?: number
  marginTop?: number
  marginBottom?: number
  flexDirection?: 'row' | 'column'
  flexGrow?: number
  flexShrink?: number
  gap?: number
  width?: number | 'auto' | `${number}%`
  height?: number | 'auto' | `${number}%`
  /** When true, renders an outer border on all sides. */
  border?: boolean
}

export function ThemedBox({
  children,
  backgroundColor,
  borderColor,
  borderStyle,
  padding,
  paddingX,
  paddingY,
  margin,
  marginX,
  marginY,
  marginTop,
  marginBottom,
  flexDirection,
  flexGrow,
  flexShrink,
  gap,
  width,
  height,
  border,
}: Props) {
  const theme = useTheme() as Theme
  const bg = resolveColor(backgroundColor) ?? theme.bg
  const border_ = resolveColor(borderColor)
  return (
    <box
      backgroundColor={bg}
      borderColor={border_}
      borderStyle={borderStyle}
      border={border}
      padding={padding}
      paddingX={paddingX}
      paddingY={paddingY}
      margin={margin}
      marginX={marginX}
      marginY={marginY}
      marginTop={marginTop}
      marginBottom={marginBottom}
      flexDirection={flexDirection}
      flexGrow={flexGrow}
      flexShrink={flexShrink}
      gap={gap}
      width={width}
      height={height}
    >
      {children}
    </box>
  )
}
