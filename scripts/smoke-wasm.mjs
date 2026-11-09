import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import init, { Simulation, execute_json, validate_json } from '../web/src/wasm/ace_cdl_runtime.js'

await init({ module_or_path: readFileSync(new URL('../web/src/wasm/ace_cdl_runtime_bg.wasm', import.meta.url)) })

for (const [file, expected] of [
  ['obc_zone_status.json', 'yEndSetUp'],
  ['vav_composite_template.json', 'damper_position'],
]) {
  const graph = JSON.parse(readFileSync(new URL(`../web/public/examples/${file}`, import.meta.url)))
  assert.deepEqual(JSON.parse(validate_json(JSON.stringify(graph))), [], `${file} graph validation`)
  const inputs = Object.fromEntries(graph.inputs.map(port => [port.name,
    port.type === 'Boolean' ? false : port.unit === '°C' ? 22 : 1]))
  const result = JSON.parse(execute_json(JSON.stringify(graph), JSON.stringify(inputs), 900))
  assert.ok(Object.hasOwn(result, expected), `${file} missing ${expected}`)
  console.log(`WASM quick test passed: ${file}`)
}

const weather = readFileSync(new URL('../web/public/examples/weather_2025.json', import.meta.url), 'utf8')
for (const file of ['four_pipe_building.json', 'four_pipe_g36.json']) {
  const graph = readFileSync(new URL(`../web/public/examples/${file}`, import.meta.url), 'utf8')
  const sim = new Simulation(graph, weather, '{}')
  try {
    const first = JSON.parse(sim.step_chunk(1))
    assert.ok(Array.isArray(first.rows), `${file} first simulation step`)
    console.log(`WASM building step passed: ${file}`)
  } finally {
    sim.free()
  }
}

const conventional = JSON.parse(readFileSync(new URL('../web/public/examples/four_pipe_building.json', import.meta.url)))
const preconditioning = conventional.blocks.find(block => block.name === 'conventional_preconditioning_off')
const occupancy = conventional.blocks.find(block => block.name === 'conventional_occupancy_false')
assert.ok(preconditioning && occupancy, 'conventional status must use independent editable CDL blocks')
preconditioning.parameters.find(parameter => parameter.name === 'k').value = true
const edited = new Simulation(JSON.stringify(conventional), weather, '{}')
try {
  const first = JSON.parse(edited.step_chunk(1)).rows[0]
  assert.equal(first.zone_presence_raw[0], 0, 'raw sensor remains exogenous')
  assert.equal(first.preconditioning_mode, 1, 'simulation uses edited CDL mode')
  assert.equal(first.zone_occupied[0], 0, 'mode edit does not change occupancy status')
} finally {
  edited.free()
}
preconditioning.parameters.find(parameter => parameter.name === 'k').value = false
occupancy.parameters.find(parameter => parameter.name === 'k').value = true
const occupancyEdited = new Simulation(JSON.stringify(conventional), weather, '{}')
try {
  const first = JSON.parse(occupancyEdited.step_chunk(1)).rows[0]
  assert.equal(first.preconditioning_mode, 0, 'occupancy edit does not change mode')
  assert.equal(first.zone_occupied[0], 1, 'simulation uses edited CDL occupancy status')
  console.log('WASM conventional CDL status output passed')
} finally {
  occupancyEdited.free()
}
