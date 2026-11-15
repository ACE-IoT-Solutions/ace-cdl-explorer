import init, { aligned_step } from '../wasm/ace_cdl_wasm.js'
import type { Block } from './types'

let ready: Promise<unknown> | undefined
export function initializeRuntime(): Promise<unknown> {
  return ready ??= init().catch(error => { ready = undefined; throw error })
}
export function alignedStepForGraph(graph: Block, requested: number): number {
  return aligned_step(JSON.stringify(graph), requested)
}
