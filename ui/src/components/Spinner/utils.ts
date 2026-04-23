import type { RGBColor } from './types.js'

/**
 * Colour and character helpers for the Spinner suite. Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/utils.ts`. The
 * algorithms (default frame set, linear RGB interpolation, HSL-to-RGB
 * conversion, rgb(...) parsing with a memoisation cache) are lifted
 * unchanged because they're pure maths with no Ink dependency.
 */

export function getDefaultCharacters(): string[] {
  if (process.env.TERM === 'xterm-ghostty') {
    // \u2731 replaces \u2733 (emoji, renders offset in Ghostty); * replaces
    // \u273d (same).
    return ['\u00b7', '\u2722', '\u2731', '\u2736', '\u273b', '*']
  }
  // \u2733 (U+2733) is matched by emoji-regex in Node.js \u2192 stringWidth
  // returns 2 instead of 1, causing layout jitter when the spinner cycles
  // frames. \u2731 is visually similar but not emoji.
  return process.platform === 'darwin'
    ? ['\u00b7', '\u2722', '\u2731', '\u2736', '\u273b', '\u273d']
    : ['\u00b7', '\u2722', '\u2731', '\u2736', '\u273b', '\u273d']
}

/** Linear interpolation between two colours in RGB space. */
export function interpolateColor(
  color1: RGBColor,
  color2: RGBColor,
  t: number,
): RGBColor {
  return {
    r: Math.round(color1.r + (color2.r - color1.r) * t),
    g: Math.round(color1.g + (color2.g - color1.g) * t),
    b: Math.round(color1.b + (color2.b - color1.b) * t),
  }
}

/** Convert an RGB record to the `#RRGGBB` hex string OpenTUI accepts. */
export function toRGBColor({ r, g, b }: RGBColor): string {
  const hex = (n: number) =>
    Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0')
  return `#${hex(r)}${hex(g)}${hex(b)}`
}

/**
 * HSL hue (0\u2013360) to RGB, using the voice-mode waveform parameters
 * (s = 0.7, l = 0.6).
 */
export function hueToRgb(hue: number): RGBColor {
  const h = ((hue % 360) + 360) % 360
  const s = 0.7
  const l = 0.6
  const cValue = (1 - Math.abs(2 * l - 1)) * s
  const x = cValue * (1 - Math.abs(((h / 60) % 2) - 1))
  const m = l - cValue / 2
  let r = 0
  let g = 0
  let b = 0
  if (h < 60) {
    r = cValue
    g = x
  } else if (h < 120) {
    r = x
    g = cValue
  } else if (h < 180) {
    g = cValue
    b = x
  } else if (h < 240) {
    g = x
    b = cValue
  } else if (h < 300) {
    r = x
    b = cValue
  } else {
    r = cValue
    b = x
  }
  return {
    r: Math.round((r + m) * 255),
    g: Math.round((g + m) * 255),
    b: Math.round((b + m) * 255),
  }
}

const RGB_CACHE = new Map<string, RGBColor | null>()

/**
 * Parse an `rgb(r, g, b)` or `#rrggbb` string into an RGB record. Results
 * are cached so rendering a spinner doesn't re-parse the same theme
 * colour on every frame.
 */
export function parseRGB(colorStr: string): RGBColor | null {
  const cached = RGB_CACHE.get(colorStr)
  if (cached !== undefined) return cached

  const rgbMatch = colorStr.match(
    /rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)/,
  )
  if (rgbMatch) {
    const out: RGBColor = {
      r: parseInt(rgbMatch[1]!, 10),
      g: parseInt(rgbMatch[2]!, 10),
      b: parseInt(rgbMatch[3]!, 10),
    }
    RGB_CACHE.set(colorStr, out)
    return out
  }

  const hexMatch = colorStr.match(/^#([0-9a-fA-F]{6})$/)
  if (hexMatch) {
    const hex = hexMatch[1]!
    const out: RGBColor = {
      r: parseInt(hex.slice(0, 2), 16),
      g: parseInt(hex.slice(2, 4), 16),
      b: parseInt(hex.slice(4, 6), 16),
    }
    RGB_CACHE.set(colorStr, out)
    return out
  }

  RGB_CACHE.set(colorStr, null)
  return null
}
