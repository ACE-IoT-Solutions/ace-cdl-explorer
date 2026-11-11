import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync } from 'node:fs'
import { reactive } from 'vue'
import { defaultScenario } from '../src/lib/types'
import { plainWorkerData, scenarioForWorker } from '../src/lib/workerPayload'

test('simulation scenario crosses the Worker boundary after nested Vue edits', () => {
  const scenario = reactive(structuredClone(defaultScenario))
  scenario.heat_pump_performance[0].heating_cop = 2.3
  scenario.heat_recovery_compressor_savings_fraction = 0.45
  scenario.heat_pump_part_load_performance[0].cop_multiplier = 0.8
  scenario.heat_pump_hot_water_cop_performance[0].cop_multiplier = 0.9
  scenario.heat_pump_hot_water_cop_performance[0].capacity_multiplier = 0.7
  scenario.heat_pump_chilled_water_cop_performance[0].water_c = 5
  scenario.terminal_design_flow[0] = 0.25
  scenario.ahu_fan_design_airflow[0] = 2.5
  scenario.ahu_fan_startup_seconds[0] = 180
  scenario.fan_speed_sensor_events.push({ ahu: 1, bias: -0.3, start_hour: 3, duration_hours: 1 })
  scenario.water_pump_design_flow_kg_s[0] = 2
  scenario.water_pump_startup_seconds[0] = 600
  scenario.water_pump_shutoff_pressure_pa[0] = 40000
  scenario.water_bypass_conductance[0] = 0.1
  scenario.water_coil_conductance[1] = 0.6
  scenario.hot_reheat_conductance = 0.3
  scenario.zone_air_volume_m3_by_zone = [100,1000,...Array(8).fill(250)]
  scenario.zone_infiltration_m3_s_by_zone = [0,0.5,...Array(8).fill(0.01)]
  scenario.open_window_additional_infiltration_m3_s_by_zone = [0,0.5,...Array(8).fill(0.2)]
  scenario.zone_design_occupants = [16,0,8,8,8,8,8,8,8,8]
  scenario.water_pump_faults.push({ loop_name: 'hot', start_hour: 7.75, duration_hours: 0.5 })
  scenario.water_flow_sensor_events.push({ loop_name: 'hot', bias_kg_s: -2, start_hour: 8, duration_hours: 1 })
  scenario.equipment_outages.push({ equipment: 'boiler', start_hour: 12, duration_hours: 2 })
  scenario.occupancy_sensor_events.push({ zone: 4, present: false, start_hour: 12.25, duration_hours: 0.5 })
  scenario.occupant_count_events.push({ zone: 4, count: 6, start_hour: 12.25, duration_hours: 0.5 })
  scenario.alarm_acknowledgements.push({ zone: 3, start_hour: 12.5, duration_hours: 0.25 })
  scenario.ahu_alarm_acknowledgements.push({ ahu: 2, start_hour: 13, duration_hours: 0.25 })
  scenario.plant_alarm_acknowledgements.push({ start_hour: 13.5, duration_hours: 0.25 })
  assert.throws(() => structuredClone({ ...scenario }), /could not be cloned/)
  const payload = scenarioForWorker(scenario)
  assert.deepEqual(structuredClone(payload), payload)
  assert.equal(payload.heat_pump_performance[0].heating_cop, 2.3)
  assert.equal(payload.heat_recovery_compressor_savings_fraction, 0.45)
  assert.equal(payload.heat_pump_part_load_performance[0].cop_multiplier, 0.8)
  assert.equal(payload.heat_pump_hot_water_cop_performance[0].cop_multiplier, 0.9)
  assert.equal(payload.heat_pump_hot_water_cop_performance[0].capacity_multiplier, 0.7)
  assert.equal(payload.heat_pump_chilled_water_cop_performance[0].water_c, 5)
  assert.equal(payload.terminal_design_flow[0], 0.25)
  assert.equal(payload.ahu_fan_design_airflow[0], 2.5)
  assert.equal(payload.ahu_fan_startup_seconds[0], 180)
  assert.deepEqual(payload.fan_speed_sensor_events[0], { ahu: 1, bias: -0.3, start_hour: 3, duration_hours: 1 })
  assert.equal(payload.water_pump_design_flow_kg_s[0], 2)
  assert.equal(payload.water_pump_startup_seconds[0], 600)
  assert.equal(payload.water_pump_shutoff_pressure_pa[0], 40000)
  assert.equal(payload.water_bypass_conductance[0], 0.1)
  assert.equal(payload.water_coil_conductance[1], 0.6)
  assert.equal(payload.hot_reheat_conductance, 0.3)
  assert.deepEqual(payload.zone_air_volume_m3_by_zone, [100,1000,...Array(8).fill(250)])
  assert.deepEqual(payload.zone_infiltration_m3_s_by_zone, [0,0.5,...Array(8).fill(0.01)])
  assert.deepEqual(payload.open_window_additional_infiltration_m3_s_by_zone, [0,0.5,...Array(8).fill(0.2)])
  assert.deepEqual(payload.zone_design_occupants, [16,0,8,8,8,8,8,8,8,8])
  assert.deepEqual(payload.water_pump_faults[0], { loop_name: 'hot', start_hour: 7.75, duration_hours: 0.5 })
  assert.deepEqual(payload.water_flow_sensor_events[0], { loop_name: 'hot', bias_kg_s: -2, start_hour: 8, duration_hours: 1 })
  assert.equal(payload.equipment_outages[0].equipment, 'boiler')
  assert.deepEqual(payload.occupancy_sensor_events[0], { zone: 4, present: false, start_hour: 12.25, duration_hours: 0.5 })
  assert.deepEqual(payload.occupant_count_events[0], { zone: 4, count: 6, start_hour: 12.25, duration_hours: 0.5 })
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
