import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { resolveSignalUnits } from '../src/lib/signalUnits'
import { physicalSignalsForGraph, signalValue } from '../src/lib/signals'
import type { Block, Row } from '../src/lib/types'

const building=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json',import.meta.url),'utf8')) as Block
const g36=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_g36.json',import.meta.url),'utf8')) as Block

test('AHU damper sensor traces remain distinct from physical actuator positions',()=>{
  const signals=physicalSignalsForGraph(g36)
  const actual=signals.find(signal=>signal.id==='physical|ahu_1_actual_outdoor_damper_position')!
  const measured=signals.find(signal=>signal.id==='physical|ahu_1_measured_outdoor_damper_position')!
  assert.equal(actual.brickClass,'Point')
  assert.equal(measured.brickClass,'Damper_Position_Sensor')
  assert.equal(actual.unit,measured.unit)
})

test('VAV reheat stem sensor traces remain distinct from physical actuator positions',()=>{
  const signals=physicalSignalsForGraph(g36)
  const actual=signals.find(signal=>signal.id==='physical|zone_01_actual_reheat_valve_position')!
  const measured=signals.find(signal=>signal.id==='physical|zone_01_measured_reheat_valve_position')!
  assert.equal(actual.brickClass,'Point')
  assert.equal(measured.brickClass,'Valve_Position_Sensor')
  assert.equal(actual.unit,measured.unit)
})

test('VAV routes use temperature, difference, Boolean, fraction, and airflow units',()=>{
  const zone=building.blocks!.find(block=>block.name==='zone_01')!
  const units=resolveSignalUnits(zone)
  assert.equal(units.get('cool_error.u1'),'°C')
  assert.equal(units.get('cool_error.y'),'K')
  assert.equal(units.get('cooling_mode.y'),'Boolean')
  assert.equal(units.get('cool_demand_gain.y'),'fraction (0–1)')
  assert.equal(units.get('flow_span.y'),'normalized airflow')
  assert.equal(units.get('heating_mode.u2'),'K')
  assert.equal(units.get('reheat_select.u2'),'fraction (0–1)')
})

test('AHU fan gain changes airflow request into a speed fraction',()=>{
  const ahu=building.blocks!.find(block=>block.name==='ahu_1')!
  const units=resolveSignalUnits(ahu)
  assert.equal(units.get('fan_gain.u'),'normalized airflow')
  assert.equal(units.get('fan_gain.y'),'fraction (0–1)')
})

test('AHU coil routes distinguish temperature error from valve commands',()=>{
  const ahu=building.blocks!.find(block=>block.name==='ahu_1')!
  const units=resolveSignalUnits(ahu)
  assert.equal(units.get('heating_coil_error.y'),'K')
  assert.equal(units.get('heating_coil_feedforward.y'),'fraction (0–1)')
  assert.equal(units.get('cooling_coil_needed.y'),'Boolean')
  assert.equal(units.get('cooling_coil_valve_position'),'fraction (0–1)')
})

test('plant pump routes distinguish load, speed, enable and water temperature',()=>{
  const plant=building.blocks!.find(block=>block.name==='plant')!
  const units=resolveSignalUnits(plant)
  assert.equal(units.get('hot_water_pump_gain.u'),'kW')
  assert.equal(units.get('hot_water_pump_gain.y'),'fraction (0–1)')
  assert.equal(units.get('hot_water_pump_needed.y'),'Boolean')
  assert.equal(units.get('hot_water_supply_setpoint.y'),'°C')
  assert.equal(units.get('hot_water_flow_kg_s'),'kg/s')
  assert.equal(units.get('hot_water_source_error.u2'),'°C')
  assert.equal(units.get('hot_water_source_error.y'),'K')
  assert.equal(units.get('hot_water_source_gain.y'),'kW')
  assert.equal(units.get('hot_water_source_nonnegative.y'),'kW')
})

test('G36 chilled-water reset routes distinguish error, requests, and temperature',()=>{
  const plant=g36.blocks!.find(block=>block.name==='plant')!
  const units=resolveSignalUnits(plant)
  assert.equal(units.get('ahu_1_supply_error.y'),'K')
  assert.equal(units.get('ahu_1_select_three_requests.y'),'count')
  assert.equal(units.get('chilled_reset_previous.y'),'fraction (0–1)')
  assert.equal(units.get('chilled_reset_warm_delta.y'),'K')
  assert.equal(units.get('chilled_water_supply_setpoint_c'),'°C')
})

test('G36 supply-air reset routes distinguish VAV requests from target temperature',()=>{
  const ahu=g36.blocks!.find(block=>block.name==='ahu_1')!
  const units=resolveSignalUnits(ahu)
  assert.equal(units.get('zone_01_sat_request'),'count')
  assert.equal(units.get('sat_request_sum_4.y'),'count')
  assert.equal(units.get('sat_response_gain.y'),'K')
  assert.equal(units.get('sat_previous.y'),'°C')
  assert.equal(units.get('sat_output.y'),'°C')
})

test('G36 terminal discharge loop separates temperatures, differences, and valve commands',()=>{
  const zone=g36.blocks!.find(block=>block.name==='zone_01')!
  const units=resolveSignalUnits(zone)
  assert.equal(units.get('discharge_max_rise.y'),'K')
  assert.equal(units.get('discharge_positive_span.y'),'K')
  assert.equal(units.get('discharge_target.y'),'°C')
  assert.equal(units.get('discharge_valve_pi.y'),'fraction (0–1)')
  assert.equal(units.get('discharge_air_setpoint_c'),'°C')
  assert.equal(zone.outputs?.find(port=>port.name==='discharge_air_setpoint_c')?.brick_class,'Discharge_Air_Temperature_Setpoint')
})

test('G36 CO₂ feedback has ppm routes and a Brick sensor plot',()=>{
  const zone=g36.blocks!.find(block=>block.name==='zone_01')!
  const units=resolveSignalUnits(zone)
  assert.equal(units.get('co2_ppm'),'ppm')
  assert.equal(units.get('co2_reset_limit.y'),'fraction (0–1)')
  assert.equal(units.get('co2_minimum_flow.y'),'normalized airflow')
  const sensor=physicalSignalsForGraph(g36).find(signal=>signal.id==='physical|zone_01_co2_ppm')
  assert.equal(sensor?.brickClass,'Zone_CO2_Level_Sensor')
  assert.equal(sensor?.unit,'ppm')
  assert.equal(signalValue({zone_c:[22],zone_co2_ppm:[900]} as Row,'physical|zone_01_co2_ppm'),900)
  const count=physicalSignalsForGraph(g36).find(signal=>signal.id==='physical|zone_01_occupant_count_actual')
  assert.equal(count?.unit,'people')
  assert.equal(signalValue({zone_c:[22],zone_occupant_count_actual:[6],zone_presence_raw:[0]} as Row,'physical|zone_01_occupant_count_actual'),6)
  const countSensor=physicalSignalsForGraph(g36).find(signal=>signal.id==='physical|zone_01_occupant_count_sensor')
  assert.equal(countSensor?.brickClass,'Occupancy_Count_Sensor')
  assert.equal(signalValue({zone_c:[22],zone_occupant_count_actual:[6],zone_occupant_count_sensor:[0]} as Row,'physical|zone_01_occupant_count_sensor'),0)
  const sensibleHeat=physicalSignalsForGraph(g36).find(signal=>signal.id==='physical|zone_01_occupant_sensible_heat_kw')
  assert.equal(sensibleHeat?.unit,'kW')
  assert.equal(signalValue({zone_c:[22],zone_occupant_sensible_heat_kw:[0.45]} as Row,'physical|zone_01_occupant_sensible_heat_kw'),0.45)
  assert.equal(physicalSignalsForGraph(building).some(signal=>signal.id==='physical|zone_01_co2_ppm'),false)
})
