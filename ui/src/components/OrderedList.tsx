import React, { createContext, isValidElement, type ReactNode, useContext } from 'react'
import { c } from '../theme.js'

/**
 * Lite-native port of the sample tree's `OrderedList` / `OrderedListItem`
 * (`ui/examples/upstream-patterns/src/components/ui/OrderedList.tsx`).
 *
 * Renders an ordered list where nested lists automatically produce
 * multi-segment markers like `1.1.` / `1.2.1.`. Consumers compose it as:
 *
 *   <OrderedList>
 *     <OrderedList.Item>first</OrderedList.Item>
 *     <OrderedList.Item>
 *       second
 *       <OrderedList>
 *         <OrderedList.Item>nested</OrderedList.Item>
 *       </OrderedList>
 *     </OrderedList.Item>
 *   </OrderedList>
 */

const OrderedListContext = createContext<{ marker: string }>({ marker: '' })
const OrderedListItemContext = createContext<{ marker: string }>({ marker: '' })

function OrderedListItem({ children }: { children: ReactNode }) {
  const { marker } = useContext(OrderedListItemContext)
  return (
    <box flexDirection="row" gap={1}>
      <text fg={c.dim}>{marker}</text>
      <box flexDirection="column">{children}</box>
    </box>
  )
}

type OrderedListProps = {
  children: ReactNode
}

function OrderedListComponent({ children }: OrderedListProps) {
  const { marker: parentMarker } = useContext(OrderedListContext)

  let itemCount = 0
  for (const child of React.Children.toArray(children)) {
    if (isValidElement(child) && child.type === OrderedListItem) {
      itemCount++
    }
  }

  const maxMarkerWidth = String(itemCount).length

  return (
    <box flexDirection="column">
      {React.Children.map(children, (child, index) => {
        if (!isValidElement(child) || child.type !== OrderedListItem) {
          return child
        }
        const paddedMarker = `${String(index + 1).padStart(maxMarkerWidth)}.`
        const marker = `${parentMarker}${paddedMarker}`
        return (
          <OrderedListContext.Provider value={{ marker }}>
            <OrderedListItemContext.Provider value={{ marker }}>
              {child}
            </OrderedListItemContext.Provider>
          </OrderedListContext.Provider>
        )
      })}
    </box>
  )
}

type OrderedListType = typeof OrderedListComponent & {
  Item: typeof OrderedListItem
}

const OrderedList = OrderedListComponent as OrderedListType
OrderedList.Item = OrderedListItem

export { OrderedList, OrderedListItem }
