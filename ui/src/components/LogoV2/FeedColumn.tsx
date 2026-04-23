import React from 'react'
import { c } from '../../theme.js'
import { calculateFeedWidth, Feed, type FeedConfig } from './Feed.js'

/**
 * Vertical column of `Feed` blocks with thin dividers between them.
 *
 * OpenTUI-native port of the upstream `LogoV2/FeedColumn`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/FeedColumn.tsx`).
 * Upstream used Ink's `<Divider>`. OpenTUI has no direct equivalent, so
 * the Lite port renders a box with only a top border at the accent
 * color as the separator.
 */

type FeedColumnProps = {
  feeds: FeedConfig[]
  maxWidth: number
}

export function FeedColumn({ feeds, maxWidth }: FeedColumnProps) {
  const feedWidths = feeds.map(feed => calculateFeedWidth(feed))
  const maxOfAllFeeds = Math.max(0, ...feedWidths)
  const actualWidth = Math.min(maxOfAllFeeds, maxWidth)

  return (
    <box flexDirection="column">
      {feeds.map((feed, index) => (
        <React.Fragment key={index}>
          <Feed config={feed} actualWidth={actualWidth} />
          {index < feeds.length - 1 && (
            <box
              width={actualWidth}
              border={['top']}
              borderStyle="single"
              borderColor={c.accent}
            />
          )}
        </React.Fragment>
      ))}
    </box>
  )
}
