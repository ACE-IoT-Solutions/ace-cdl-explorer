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

const lowThreshold = {
  name:'minimum_discharge',block_type:'Buildings.Controls.OBC.CDL.Reals.LessThreshold',
  inputs:[{name:'u',type:'Real'}],outputs:[{name:'y',type:'Boolean'}],
  parameters:[{name:'t',value:10},{name:'h',value:0.25}],
}
assert.deepEqual(JSON.parse(validate_json(JSON.stringify(lowThreshold))), [], 'LessThreshold validation')
for(const [value,expected] of [[9.9,true],[10.1,false]]){
  const actual=JSON.parse(execute_json(JSON.stringify(lowThreshold),JSON.stringify({u:value}),60))
  assert.equal(actual.y,expected,`WASM LessThreshold at ${value} °C`)
}
console.log('WASM CDL LessThreshold passed')

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

const buildingGraph = readFileSync(new URL('../web/public/examples/four_pipe_building.json', import.meta.url), 'utf8')
const designRuns = [
  new Simulation(buildingGraph, weather, '{}'),
  new Simulation(buildingGraph, weather, JSON.stringify({terminal_design_flow:[0.25,...Array(9).fill(0.95)]})),
]
try {
  const [normal, reduced] = designRuns.map(sim => JSON.parse(sim.step_chunk(36)).rows[35])
  assert.ok(normal.zone_airflow[0] > reduced.zone_airflow[0], 'WASM physical VAV capacity changes delivered flow')
  console.log('WASM per-zone VAV design airflow passed')
} finally {
  for (const sim of designRuns) sim.free()
}

const fanRuns = [
  new Simulation(buildingGraph, weather, '{}'),
  new Simulation(buildingGraph, weather, JSON.stringify({ahu_fan_design_airflow:[1,5]})),
]
try {
  const [normal, reduced] = fanRuns.map(sim => JSON.parse(sim.step_chunk(36)).rows[35])
  assert.ok(normal.zone_airflow[0] > reduced.zone_airflow[0], 'WASM AHU physical fan capacity changes delivered flow')
  console.log('WASM per-AHU fan design airflow passed')
} finally {
  for (const sim of fanRuns) sim.free()
}

const pumpRuns = [
  new Simulation(buildingGraph, weather, '{}'),
  new Simulation(buildingGraph, weather, JSON.stringify({
    water_pump_design_flow_kg_s:[2,4.31], water_pump_shutoff_pressure_pa:[40000,80000],
  })),
]
try {
  const [normal, reduced] = pumpRuns.map(sim => JSON.parse(sim.step_chunk(36)).rows[35])
  assert.ok(normal.hot_water_flow_kg_s > reduced.hot_water_flow_kg_s,
    'WASM physical pump curve changes delivered hot-water flow')
  assert.ok(normal.hot_water_differential_pressure_pa > reduced.hot_water_differential_pressure_pa,
    'WASM physical pump curve changes measured remote pressure')
  console.log('WASM physical water pump curves passed')
} finally {
  for (const sim of pumpRuns) sim.free()
}

const g36 = JSON.parse(readFileSync(new URL('../web/public/examples/four_pipe_g36.json', import.meta.url)))
const vav = g36.blocks.find(block => block.name === 'zone_01')
const vavInputs = Object.fromEntries(vav.inputs.map(port => [port.name,
  port.type === 'Boolean' ? false : port.type === 'Integer' ? 0 : 22]))
for (const [code, expected] of [[1, 0], [2, 0.95], [3, 0.2], [4, 0.95]]) {
  vavInputs.flow_override_code = code
  const result = JSON.parse(execute_json(JSON.stringify(vav), JSON.stringify(vavInputs), 60))
  assert.equal(result.airflow_request, expected, `WASM G36 VAV override code ${code}`)
}
console.log('WASM G36 VAV airflow overrides passed')

const conventional = JSON.parse(readFileSync(new URL('../web/public/examples/four_pipe_building.json', import.meta.url)))
const conventionalPlant = structuredClone(conventional.blocks.find(block => block.name === 'plant'))
const plantInputs = Object.fromEntries(conventionalPlant.inputs.map(port => [port.name, 0]))
plantInputs.heat_request_kw = 100
plantInputs.cool_request_kw = 100
const modeOutput = graph => JSON.parse(execute_json(JSON.stringify(graph), JSON.stringify(plantInputs), 900)).heat_pump_requested_mode
assert.equal(modeOutput(conventionalPlant), 3, 'WASM conventional dispatch selects simultaneous mode in CDL')
conventionalPlant.connections = conventionalPlant.connections.filter(edge => edge.to !== 'heat_pump_requested_mode')
conventionalPlant.connections.push({from:'heat_pump_heating_mode_code.y',to:'heat_pump_requested_mode'})
assert.equal(modeOutput(conventionalPlant), 1, 'WASM dispatch follows an edited CDL mode wire')
console.log('WASM conventional CDL heat-pump mode passed')
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
const plant = conventional.blocks.find(block => block.name === 'plant')
plant.blocks.find(block => block.name === 'conventional_heat_pump_start_fault_off')
  .parameters.find(parameter => parameter.name === 'k').value = true
const faultEdited = new Simulation(JSON.stringify(conventional), weather, '{}')
try {
  const first = JSON.parse(faultEdited.step_chunk(1)).rows[0]
  assert.equal(first.plant_heat_pump_start_fault, 1, 'simulation reports edited CDL fault status')
  console.log('WASM conventional CDL source fault status passed')
} finally {
  faultEdited.free()
}
