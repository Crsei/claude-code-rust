import React, { useCallback, useMemo, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'

/**
 * Ported from `ui/examples/upstream-patterns/src/components/ui/TreeSelect.tsx`.
 *
 * Upstream delegated the actual keyboard handling to its `Select`
 * component (from `CustomSelect/select`); OpenTUI doesn't ship the
 * exact same `Select` API so this port inlines the arrow-key navigation
 * using `useKeyboard`. The external contract (`nodes`, `onSelect`,
 * `onCancel`, expand/collapse callbacks, focus tracking, custom
 * prefixes) stays the same so call sites can mix-and-match.
 *
 * Left/Right expand or collapse the focused node; Up/Down move focus;
 * Enter selects; Esc cancels. Controlled expand state (`isNodeExpanded`
 * + `onExpand` / `onCollapse`) takes precedence over internal state.
 */

export type TreeNode<T> = {
  id: string | number
  value: T
  label: string
  description?: string
  dimDescription?: boolean
  children?: TreeNode<T>[]
  metadata?: Record<string, unknown>
}

type FlattenedNode<T> = {
  node: TreeNode<T>
  depth: number
  isExpanded: boolean
  hasChildren: boolean
  parentId?: string | number
}

export type TreeSelectProps<T> = {
  readonly nodes: TreeNode<T>[]
  readonly onSelect: (node: TreeNode<T>) => void
  readonly onCancel?: () => void
  readonly onFocus?: (node: TreeNode<T>) => void
  readonly focusNodeId?: string | number
  readonly visibleOptionCount?: number
  readonly isDisabled?: boolean
  readonly hideIndexes?: boolean
  readonly isNodeExpanded?: (nodeId: string | number) => boolean
  readonly onExpand?: (nodeId: string | number) => void
  readonly onCollapse?: (nodeId: string | number) => void
  readonly getParentPrefix?: (isExpanded: boolean) => string
  readonly getChildPrefix?: (depth: number) => string
  readonly onUpFromFirstItem?: () => void
}

const DEFAULT_PARENT = (expanded: boolean) => (expanded ? '\u25BC ' : '\u25B6 ')
const DEFAULT_CHILD = (_depth: number) => '  \u25B8 '

export function TreeSelect<T>({
  nodes,
  onSelect,
  onCancel,
  onFocus,
  focusNodeId,
  visibleOptionCount,
  isDisabled = false,
  isNodeExpanded,
  onExpand,
  onCollapse,
  getParentPrefix,
  getChildPrefix,
  onUpFromFirstItem,
}: TreeSelectProps<T>): React.ReactElement {
  const [internalExpandedIds, setInternalExpandedIds] = useState<
    Set<string | number>
  >(new Set())

  const parentPrefixFn = getParentPrefix ?? DEFAULT_PARENT
  const childPrefixFn = getChildPrefix ?? DEFAULT_CHILD

  const isExpanded = useCallback(
    (nodeId: string | number): boolean => {
      if (isNodeExpanded) return isNodeExpanded(nodeId)
      return internalExpandedIds.has(nodeId)
    },
    [isNodeExpanded, internalExpandedIds],
  )

  const flattenedNodes = useMemo<FlattenedNode<T>[]>(() => {
    const result: FlattenedNode<T>[] = []
    const traverse = (
      node: TreeNode<T>,
      depth: number,
      parentId?: string | number,
    ): void => {
      const hasChildren = !!node.children && node.children.length > 0
      const nodeIsExpanded = isExpanded(node.id)
      result.push({ node, depth, isExpanded: nodeIsExpanded, hasChildren, parentId })
      if (hasChildren && nodeIsExpanded && node.children) {
        for (const child of node.children) traverse(child, depth + 1, node.id)
      }
    }
    for (const node of nodes) traverse(node, 0)
    return result
  }, [nodes, isExpanded])

  const nodeMap = useMemo(() => {
    const map = new Map<string | number, TreeNode<T>>()
    for (const fn of flattenedNodes) map.set(fn.node.id, fn.node)
    return map
  }, [flattenedNodes])

  const [focusIndex, setFocusIndex] = useState<number>(() => {
    if (focusNodeId === undefined) return 0
    const idx = flattenedNodes.findIndex(fn => fn.node.id === focusNodeId)
    return idx >= 0 ? idx : 0
  })
  const lastFocusedIdRef = useRef<string | number | null>(null)

  const focusNode = useCallback(
    (index: number) => {
      const clamped = Math.max(0, Math.min(flattenedNodes.length - 1, index))
      setFocusIndex(clamped)
      const entry = flattenedNodes[clamped]
      if (!entry) return
      if (lastFocusedIdRef.current === entry.node.id) return
      lastFocusedIdRef.current = entry.node.id
      onFocus?.(entry.node)
    },
    [flattenedNodes, onFocus],
  )

  const toggleExpand = useCallback(
    (nodeId: string | number, shouldExpand: boolean) => {
      if (shouldExpand) {
        if (onExpand) {
          onExpand(nodeId)
        } else {
          setInternalExpandedIds(prev => new Set(prev).add(nodeId))
        }
      } else if (onCollapse) {
        onCollapse(nodeId)
      } else {
        setInternalExpandedIds(prev => {
          const next = new Set(prev)
          next.delete(nodeId)
          return next
        })
      }
    },
    [onExpand, onCollapse],
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (isDisabled) return
    const name = event.name

    if (name === 'escape') {
      onCancel?.()
      return
    }

    if (flattenedNodes.length === 0) return
    const current = flattenedNodes[focusIndex]
    if (!current) return

    if (name === 'up') {
      if (focusIndex === 0) {
        onUpFromFirstItem?.()
        return
      }
      focusNode(focusIndex - 1)
      return
    }
    if (name === 'down') {
      focusNode(focusIndex + 1)
      return
    }
    if (name === 'right' && current.hasChildren && !current.isExpanded) {
      toggleExpand(current.node.id, true)
      return
    }
    if (name === 'left') {
      if (current.hasChildren && current.isExpanded) {
        toggleExpand(current.node.id, false)
      } else if (current.parentId !== undefined) {
        toggleExpand(current.parentId, false)
        const parentIdx = flattenedNodes.findIndex(
          fn => fn.node.id === current.parentId,
        )
        if (parentIdx >= 0) focusNode(parentIdx)
      }
      return
    }
    if (name === 'return' || name === 'enter') {
      const node = nodeMap.get(current.node.id)
      if (node) onSelect(node)
    }
  })

  const visibleCount = visibleOptionCount ?? flattenedNodes.length
  const windowStart = Math.min(
    Math.max(0, focusIndex - Math.floor(visibleCount / 2)),
    Math.max(0, flattenedNodes.length - visibleCount),
  )
  const windowEnd = Math.min(flattenedNodes.length, windowStart + visibleCount)
  const visibleNodes = flattenedNodes.slice(windowStart, windowEnd)

  return (
    <box flexDirection="column">
      {visibleNodes.map((flatNode, offset) => {
        const index = windowStart + offset
        const isFocused = index === focusIndex
        let prefix = ''
        if (flatNode.hasChildren) {
          prefix = parentPrefixFn(flatNode.isExpanded)
        } else if (flatNode.depth > 0) {
          prefix = childPrefixFn(flatNode.depth)
        }
        return (
          <box
            key={String(flatNode.node.id)}
            flexDirection="row"
          >
            <text
              fg={isFocused ? c.bg : c.text}
              bg={isFocused ? c.textBright : undefined}
            >
              {isFocused ? '\u25B8 ' : '  '}
              {prefix}
              {flatNode.node.label}
            </text>
            {flatNode.node.description ? (
              <text
                fg={
                  flatNode.node.dimDescription === false && isFocused
                    ? c.text
                    : c.dim
                }
              >
                {' '}
                {flatNode.node.description}
              </text>
            ) : null}
          </box>
        )
      })}
    </box>
  )
}
