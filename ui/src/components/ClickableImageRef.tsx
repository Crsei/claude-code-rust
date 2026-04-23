import React from 'react'
import { pathToFileURL } from 'node:url'
import { c } from '../theme.js'
import { osc8Link } from './FilePathLink.js'

/**
 * Render an `[Image #N]` reference as an OSC 8 hyperlink.
 *
 * OpenTUI-native port of the upstream `ClickableImageRef`
 * (`ui/examples/upstream-patterns/src/components/ClickableImageRef.tsx`).
 * Upstream resolved the stored image path via `getStoredImagePath(id)`.
 * The Lite port accepts the resolved path as a prop so the frontend
 * stays decoupled from the Rust-side image store. When no path is
 * available, the component falls back to styled text \u2014 still visible,
 * but not clickable.
 */

type Props = {
  imageId: number
  /** Absolute path on disk (resolved by the backend image store). */
  imagePath?: string | null
  /** Highlight background, e.g. for message selectors. */
  backgroundColor?: string
  /** Render in inverse (selected) styling. */
  isSelected?: boolean
}

export function ClickableImageRef({
  imageId,
  imagePath,
  backgroundColor,
  isSelected = false,
}: Props) {
  const displayText = `[Image #${imageId}]`
  const invertedFg = isSelected ? c.bg : undefined
  const invertedBg = isSelected ? c.text : backgroundColor

  if (imagePath) {
    const href = pathToFileURL(imagePath).href
    const payload = osc8Link(href, displayText)
    return (
      <text fg={invertedFg} bg={invertedBg}>
        {isSelected ? <strong>{payload}</strong> : payload}
      </text>
    )
  }

  return (
    <text fg={invertedFg} bg={invertedBg}>
      {isSelected ? <strong>{displayText}</strong> : displayText}
    </text>
  )
}
