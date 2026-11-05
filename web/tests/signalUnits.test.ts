import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { resolveSignalUnits } from '../src/lib/signalUnits'
import type { Block } from '../src/lib/types'

const building=JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json',import.meta.url),'utf8')) as Block

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
