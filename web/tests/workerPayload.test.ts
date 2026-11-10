import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync } from 'node:fs'
import { reactive } from 'vue'
import { defaultScenario } from '../src/lib/types'
import { plainWorkerData, scenarioForWorker } from '../src/lib/workerPayload'

test('simulation scenario crosses the Worker boundary after nested Vue edits', () => {
  const scenario = reactive(structuredClone(defaultScenario))
  scenario.heat_pump_performance[0].heating_cop = 2.3
  scenario.terminal_design_flow[0] = 0.25
  scenario.ahu_fan_design_airflow[0] = 2.5
  scenario.equipment_outages.push({ equipment: 'boiler', start_hour: 12, duration_hours: 2 })
  scenario.alarm_acknowledgements.push({ zone: 3, start_hour: 12.5, duration_hours: 0.25 })
  scenario.ahu_alarm_acknowledgements.push({ ahu: 2, start_hour: 13, duration_hours: 0.25 })
  scenario.plant_alarm_acknowledgements.push({ start_hour: 13.5, duration_hours: 0.25 })
  assert.throws(() => structuredClone({ ...scenario }), /could not be cloned/)
  const payload = scenarioForWorker(scenario)
  assert.deepEqual(structuredClone(payload), payload)
  assert.equal(payload.heat_pump_performance[0].heating_cop, 2.3)
  assert.equal(payload.terminal_design_flow[0], 0.25)
  assert.equal(payload.ahu_fan_design_airflow[0], 2.5)
  assert.equal(payload.equipment_outages[0].equipment, 'boiler')
  assert.deepEqual(payload.alarm_acknowledgements[0], { zone: 3, start_hour: 12.5, duration_hours: 0.25 })
  assert.deepEqual(payload.ahu_alarm_acknowledgements[0], { ahu: 2, start_hour: 13, duration_hours: 0.25 })
  assert.deepEqual(payload.plant_alarm_acknowledgements[0], { start_hour: 13.5, duration_hours: 0.25 })

  for (const example of ['four_pipe_building.json', 'four_pipe_g36.json']) {
    const graph = JSON.parse(readFileSync(new URL(`../public/examples/${example}`, import.meta.url), 'utf8'))
    const request = { id: 1, type: 'simulate', graph, scenario: payload, weather: [{ outdoor_c: 20 }], traces: [] }
    assert.deepEqual(structuredClone(request), request, `${example} request must cross the Worker boundary`)
  }
})

test('edited graph and quick-test inputs cross the Worker boundary', () => {
  const graph = reactive(JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json', import.meta.url), 'utf8')))
  const inputs = reactive({ room_temp: 23, occupancy: true })
  graph.blocks[0].notes = 'Edited in Vue'
  assert.throws(() => structuredClone(graph), /could not be cloned/)
  assert.throws(() => structuredClone(inputs), /could not be cloned/)
  const request = { id: 1, type: 'execute', graph: plainWorkerData(graph), inputs: plainWorkerData(inputs) }
  assert.deepEqual(structuredClone(request), request)
  assert.equal(request.graph.blocks[0].notes, 'Edited in Vue')
})
