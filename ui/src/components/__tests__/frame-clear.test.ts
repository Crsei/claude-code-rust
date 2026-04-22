import { expect, test } from 'bun:test'
import { registerOpaqueFrameClear } from '../frame-clear.js'

test('registerOpaqueFrameClear clears the next frame buffer to opaque black', async () => {
  let callback: (() => Promise<void>) | null = null
  const clears: [number, number, number, number][] = []

  const cleanup = registerOpaqueFrameClear({
    nextRenderBuffer: {
      clear(color) {
        clears.push(color.toInts())
      },
    },
    setFrameCallback(cb) {
      callback = cb
    },
    removeFrameCallback(cb) {
      if (callback === cb) {
        callback = null
      }
    },
  })

  expect(callback).not.toBeNull()
  await callback?.()
  expect(clears).toEqual([[0, 0, 0, 255]])

  cleanup()
  expect(callback).toBeNull()
})

