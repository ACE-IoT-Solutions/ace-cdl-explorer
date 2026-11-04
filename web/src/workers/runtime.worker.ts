import init, { Simulation, execute_json, validate_json } from '../wasm/ace_cdl_runtime.js'

let ready: Promise<unknown> | undefined
let runGeneration = 0
const ensureReady = () => ready ??= init()

self.onmessage = async ({ data }: MessageEvent) => {
  const { id, type } = data
  if (type === 'cancel') { runGeneration++; return }
  try {
    await ensureReady()
    if (type === 'validate') {
      self.postMessage({ id, type: 'validation', errors: JSON.parse(validate_json(JSON.stringify(data.graph))) })
    } else if (type === 'execute') {
      self.postMessage({ id, type: 'result', result: JSON.parse(execute_json(JSON.stringify(data.graph), JSON.stringify(data.inputs), 900)) })
    } else if (type === 'simulate') {
      const generation = ++runGeneration
      const sim = new Simulation(JSON.stringify(data.graph), JSON.stringify(data.weather), JSON.stringify(data.scenario))
      try {
        sim.set_traces(JSON.stringify(data.traces ?? []))
        while (generation === runGeneration) {
          const bytes = sim.step_chunk_arrow(672)
          self.postMessage({ id, type: 'batch', buffer: bytes.buffer, progress: sim.progress(), total: sim.total(), done: sim.done(), summary: JSON.parse(sim.summary_json()) }, [bytes.buffer])
          if (sim.done()) break
          await new Promise(resolve => setTimeout(resolve, 0))
        }
        if (generation !== runGeneration) self.postMessage({ id, type: 'cancelled' })
      } finally {
        sim.free()
      }
    }
  } catch (error) {
    self.postMessage({ id, type: 'error', message: String(error) })
  }
}
