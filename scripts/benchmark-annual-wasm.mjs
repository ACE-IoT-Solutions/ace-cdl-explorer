// Compare full-year wasm simulations built with the same compiler/build settings.
// Usage: node scripts/benchmark-annual-wasm.mjs BASELINE_PKG CANDIDATE_PKG
import assert from 'node:assert/strict'
import { readFileSync, existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { performance } from 'node:perf_hooks'

const directories = process.argv.slice(2)
assert.equal(directories.length, 2, 'provide baseline and candidate wasm package directories')
const graph = readFileSync(new URL('../web/public/examples/four_pipe_g36.json', import.meta.url), 'utf8')
const weather = readFileSync(new URL('../web/public/examples/weather_2025.json', import.meta.url), 'utf8')
const timings = [], summaries = []
for (const [index, directory] of directories.entries()) {
  const name = existsSync(resolve(directory, 'ace_cdl_wasm.js')) ? 'ace_cdl_wasm' : 'ace_cdl_runtime'
  const pkg = await import(pathToFileURL(resolve(directory, `${name}.js`)).href)
  await pkg.default({ module_or_path: readFileSync(resolve(directory, `${name}_bg.wasm`)) })
  const start = performance.now()
  const sim = new pkg.Simulation(graph, weather,
    '{"zone_ua_kw_per_k":0.15,"zone_capacity_kwh_per_k":5,"step_seconds":900}')
  let intervals = 0
  try {
    for (;;) {
      const batch = JSON.parse(sim.step_chunk(672))
      intervals += batch.rows.length
      if (batch.done) {
        assert.ok(batch.summary && typeof batch.summary === 'object', 'missing final summary')
        summaries.push(batch.summary)
        break
      }
    }
  } finally {
    sim.free()
  }
  const seconds = (performance.now() - start) / 1000
  timings.push(seconds)
  assert.equal(intervals, 35040)
  console.log(`${index ? 'candidate' : 'baseline'}: ${seconds.toFixed(3)}s for ${intervals} weather intervals`)
}
assert.deepEqual(summaries[1], summaries[0], 'full-year wasm summaries differ')
const ratio = timings[1] / timings[0]
console.log(`Exact summary parity; candidate/baseline runtime ratio ${ratio.toFixed(3)}`)
assert.ok(ratio <= 1.05, 'candidate exceeds the five-percent wasm performance limit')
