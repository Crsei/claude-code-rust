import React, { createContext, type ReactNode, useContext } from 'react'
import { c } from '../../theme.js'

/**
 * Single item inside an `<OrderedList>`. Ported from
 * `ui/examples/upstream-patterns/src/components/ui/OrderedListItem.tsx`.
 *
 * The context is populated by `<OrderedList>` which walks its children
 * and assigns each `<OrderedListItem>` a `1.`, `1.1.`, `1.2.1.` marker.
 * Kept in a separate file (matching the example layout) so `<OrderedList>`
 * can identify items by reference (`child.type === OrderedListItem`).
 */

export const OrderedListItemContext = createContext({ marker: '' })

type OrderedListItemProps = {
  children: ReactNode
}

export function OrderedListItem({
  children,
}: OrderedListItemProps): React.ReactElement {
  const { marker } = useContext(OrderedListItemContext)

  return (
    <box flexDirection="row" gap={1}>
      <text fg={c.dim}>{marker}</text>
      <box flexDirection="column">{children}</box>
    </box>
  )
}
