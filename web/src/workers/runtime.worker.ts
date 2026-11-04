import init, { Simulation, execute_json, validate_json } from '../wasm/ace_cdl_runtime.js'

let ready: Promise<unknown> | undefined
let cancelled = false
const ensureReady = () => ready ??= init()

self.onmessage = async ({ data }: MessageEvent) => {
  const { id, type } = data
  if (type === 'cancel') { cancelled = true; return }
  try {
    await ensureReady()
    if (type === 'validate') {
      self.postMessage({ id, type: 'validation', errors: JSON.parse(validate_json(JSON.stringify(data.graph))) })
    } else if (type === 'execute') {
      self.postMessage({ id, type: 'result', result: JSON.parse(execute_json(JSON.stringify(data.graph), JSON.stringify(data.inputs), 900)) })
    } else if (type === 'simulate') {
      cancelled = false
      const sim = new Simulation(JSON.stringify(data.graph), JSON.stringify(data.weather), JSON.stringify(data.scenario))
      sim.set_traces(JSON.stringify(data.traces ?? []))
      while (!cancelled) {
        const batch = JSON.parse(sim.step_chunk(672))
        self.postMessage({ id, type: 'batch', ...batch })
        if (batch.done) break
        await new Promise(resolve => setTimeout(resolve, 0))
      }
      if (cancelled) self.postMessage({ id, type: 'cancelled' })
      sim.free()
    }
  } catch (error) {
    self.postMessage({ id, type: 'error', message: String(error) })
  }
}
