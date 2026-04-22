import { expect, test } from 'bun:test'
import {
  clearTerminalBuffers,
  ensureOpaqueRendererBackground,
  notifyBackendResize,
  repaintAfterResize,
} from '../resize-sync.js'

test('notifyBackendResize forwards the new terminal size', () => {
  const sent: unknown[] = []

  notifyBackendResize(
    {
      send(message) {
        sent.push(message)
      },
    },
    160,
    48,
  )

  expect(sent).toEqual([{ type: 'resize', cols: 160, rows: 48 }])
})

test('clearTerminalBuffers clears both render buffers to opaque black', () => {
  const currentCalls: [number, number, number, number][] = []
  const nextCalls: [number, number, number, number][] = []

  clearTerminalBuffers({
    setBackgroundColor() {},
    currentRenderBuffer: {
      clear(color) {
        currentCalls.push(color.toInts())
      },
    },
    nextRenderBuffer: {
      clear(color) {
        nextCalls.push(color.toInts())
      },
    },
  })

  expect(currentCalls).toEqual([[0, 0, 0, 255]])
  expect(nextCalls).toEqual([[0, 0, 0, 255]])
})

test('ensureOpaqueRendererBackground sets the renderer background to opaque black', () => {
  const backgroundCalls: [number, number, number, number][] = []

  ensureOpaqueRendererBackground({
    setBackgroundColor(color) {
      backgroundCalls.push(color.toInts())
    },
    currentRenderBuffer: {
      clear() {},
    },
    nextRenderBuffer: {
      clear() {},
    },
    intermediateRender() {},
  })

  expect(backgroundCalls).toEqual([[0, 0, 0, 255]])
})

test('repaintAfterResize clears the terminal surface and forces an immediate render', () => {
  const clearTerminalCalls: unknown[] = []
  const backgroundCalls: [number, number, number, number][] = []
  const currentCalls: [number, number, number, number][] = []
  const nextCalls: [number, number, number, number][] = []
  let rerenders = 0

  repaintAfterResize({
    rendererPtr: 'renderer-ptr',
    lib: {
      clearTerminal(rendererPtr) {
        clearTerminalCalls.push(rendererPtr)
      },
    },
    setBackgroundColor(color) {
      backgroundCalls.push(color.toInts())
    },
    currentRenderBuffer: {
      clear(color) {
        currentCalls.push(color.toInts())
      },
    },
    nextRenderBuffer: {
      clear(color) {
        nextCalls.push(color.toInts())
      },
    },
    intermediateRender() {
      rerenders += 1
    },
  })

  expect(clearTerminalCalls).toEqual(['renderer-ptr'])
  expect(backgroundCalls).toEqual([[0, 0, 0, 255]])
  expect(currentCalls).toEqual([[0, 0, 0, 255]])
  expect(nextCalls).toEqual([[0, 0, 0, 255]])
  expect(rerenders).toBe(1)
})
