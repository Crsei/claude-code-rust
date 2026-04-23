import React, {
  createContext,
  isValidElement,
  type ReactNode,
  useContext,
} from 'react'
import { OrderedListItem, OrderedListItemContext } from './OrderedListItem.js'

/**
 * Ported from `ui/examples/upstream-patterns/src/components/ui/OrderedList.tsx`.
 *
 * Renders its children as a numbered list. Nested `<OrderedList>`s
 * combine markers — so inside a top-level item marked `1.` a nested
 * list's first item will render with `1.1.`, the next with `1.2.`, and
 * so on.
 *
 * The component walks its children once to measure the marker width so
 * single- and double-digit items line up in the gutter. Non-`<OrderedListItem>`
 * children are passed through untouched (upstream does the same).
 */

const OrderedListContext = createContext({ marker: '' })

type OrderedListProps = {
  children: ReactNode
}

function OrderedListComponent({ children }: OrderedListProps): React.ReactElement {
  const { marker: parentMarker } = useContext(OrderedListContext)

  let numberOfItems = 0
  for (const child of React.Children.toArray(children)) {
    if (!isValidElement(child) || child.type !== OrderedListItem) continue
    numberOfItems += 1
  }

  const maxMarkerWidth = String(numberOfItems).length

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
