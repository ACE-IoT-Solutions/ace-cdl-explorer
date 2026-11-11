import assert from 'node:assert/strict'
import { test } from 'node:test'
import { tableFromArrays, tableToIPC } from 'apache-arrow'
import { ArrowRows, measures } from '../src/lib/frames'

test('Arrow inspection decodes equipment status and capacity columns', () => {
  const values: Record<string, Float64Array | Uint32Array> = { index: new Uint32Array([0]) }
  for (const name of measures) values[name] = new Float64Array([0])
  for (let zone = 1; zone <= 10; zone++) {
    for (const suffix of ['c', 'co2_ppm', 'occupied', 'presence_raw', 'airflow', 'discharge_c', 'actual_damper_position', 'actual_reheat_valve_position']) {
      values[`zone_${String(zone).padStart(2, '0')}_${suffix}`] = new Float64Array([0])
    }
  }
  values.heat_pump_available_capacity_kw = new Float64Array([44])
  values.heat_pump_part_load_fraction = new Float64Array([0.5])
  values.heat_pump_cop_multiplier = new Float64Array([0.9])
  values.heat_pump_hot_water_cop_multiplier = new Float64Array([0.8])
  values.heat_pump_chilled_water_cop_multiplier = new Float64Array([1.1])
  values.heat_pump_heating_cop = new Float64Array([2.2])
  values.heat_pump_available = new Float64Array([0])
  values.boiler_available = new Float64Array([1])
  values.heat_pump_running = new Float64Array([0])
  values.hot_water_plant_proven = new Float64Array([1])
  values.zone_01_actual_damper_position = new Float64Array([0.6])
  values.zone_01_co2_ppm = new Float64Array([850])
  values.zone_01_actual_reheat_valve_position = new Float64Array([0.2])
  values.ahu_1_actual_fan_speed = new Float64Array([0.3])
  values.ahu_1_measured_fan_speed = new Float64Array([0.1])
  values.ahu_1_actual_outdoor_damper_position = new Float64Array([0.7])
  values.ahu_1_actual_return_damper_position = new Float64Array([0.2])
  values.hot_water_actual_flow_kg_s = new Float64Array([2.0])
  values.hot_water_bypass_flow_kg_s = new Float64Array([0.4])
  values.ahu_1_hot_water_flow_kg_s = new Float64Array([0.5])
  values.ahu_2_hot_water_flow_kg_s = new Float64Array([0.3])
  values.hot_water_reheat_flow_kg_s = new Float64Array([0.8])
  const buffer = tableToIPC(tableFromArrays(values), 'stream')
  const rows = new ArrowRows([])
  rows.append(buffer.slice().buffer)
  const row = rows.at(0)!
  assert.equal(row.heat_pump_available_capacity_kw, 44)
  assert.equal(row.heat_pump_part_load_fraction, 0.5)
  assert.equal(row.heat_pump_cop_multiplier, 0.9)
  assert.equal(row.heat_pump_hot_water_cop_multiplier, 0.8)
  assert.equal(row.heat_pump_chilled_water_cop_multiplier, 1.1)
  assert.equal(row.heat_pump_heating_cop, 2.2)
  assert.equal(row.heat_pump_available, 0)
  assert.equal(row.boiler_available, 1)
  assert.equal(row.heat_pump_running, 0)
  assert.equal(row.hot_water_plant_proven, 1)
  assert.equal(row.zone_actual_damper_position?.[0], 0.6)
  assert.equal(row.zone_co2_ppm?.[0], 850)
  assert.equal(row.zone_actual_reheat_valve_position?.[0], 0.2)
  assert.equal(row.ahu_1_actual_fan_speed, 0.3)
  assert.equal(row.ahu_1_measured_fan_speed, 0.1)
  assert.equal(row.ahu_1_actual_outdoor_damper_position, 0.7)
  assert.equal(row.ahu_1_actual_return_damper_position, 0.2)
  assert.equal(row.hot_water_actual_flow_kg_s, 2)
  assert.equal(row.hot_water_bypass_flow_kg_s, 0.4)
  const branches=(row.ahu_1_hot_water_flow_kg_s??0)+(row.ahu_2_hot_water_flow_kg_s??0)+(row.hot_water_reheat_flow_kg_s??0)+(row.hot_water_bypass_flow_kg_s??0)
  assert.ok(Math.abs(branches-row.hot_water_actual_flow_kg_s!)<1e-12)
})
