import { RGBA, type CliRenderer } from '@opentui/core'

const FRAME_BG = RGBA.fromInts(0, 0, 0, 255)

type BufferLike = {
  clear(color: RGBA): void
}

type FrameClearRenderer = Pick<CliRenderer, 'nextRenderBuffer' | 'setFrameCallback' | 'removeFrameCallback'>

export function registerOpaqueFrameClear(renderer: FrameClearRenderer): () => void {
  const clearFrame = async () => {
    renderer.nextRenderBuffer.clear(FRAME_BG)
  }

  renderer.setFrameCallback(clearFrame)
  return () => {
    renderer.removeFrameCallback(clearFrame)
  }
}

