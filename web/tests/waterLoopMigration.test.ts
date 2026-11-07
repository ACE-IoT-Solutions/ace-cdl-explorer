import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { migrateWaterLoopControl } from '../src/lib/waterLoopMigration'
import { defaultScenario, edgeEndpoints, type Block, type Project } from '../src/lib/types'

const reference = JSON.parse(readFileSync(new URL('../public/examples/four_pipe_building.json', import.meta.url), 'utf8')) as Block
const sensors = new Set(['hot_water_supply_temp_c','hot_water_return_temp_c','chilled_water_supply_temp_c','chilled_water_return_temp_c','hot_water_flow_kg_s','chilled_water_flow_kg_s'])
const commands = new Set(['hot_water_enable','chilled_water_enable','hot_water_pump_speed','chilled_water_pump_speed','hot_water_supply_setpoint_c','chilled_water_supply_setpoint_c'])
const addedBlocks = new Set(['pump_zero','pump_start_threshold','hot_water_pump_needed','chilled_water_pump_needed','hot_water_pump_gain','chilled_water_pump_gain','hot_water_pump_limit','chilled_water_pump_limit','hot_water_pump_select','chilled_water_pump_select','hot_water_supply_setpoint','chilled_water_supply_setpoint'])

for (const file of ['four_pipe_building.json','four_pipe_g36.json']) {
  test(`${file} saved plant gains graph-owned water pumps and measured points`, () => {
    const old = JSON.parse(readFileSync(new URL(`../public/examples/${file}`, import.meta.url), 'utf8')) as Block
    old.inputs = old.inputs?.filter(port => !sensors.has(port.name))
    old.outputs = old.outputs?.filter(port => !commands.has(port.name.replace(/^plant_/,'')))
    old.connections = old.connections?.filter(edge => {
      const [from,to] = edgeEndpoints(edge)
      return !sensors.has(from) && !commands.has(to.replace(/^plant_/,''))
    })
    const plant = old.blocks?.find(block => block.name === 'plant')!
    plant.inputs = plant.inputs?.filter(port => !sensors.has(port.name))
    plant.outputs = plant.outputs?.filter(port => !commands.has(port.name))
    plant.blocks = plant.blocks?.filter(block => !addedBlocks.has(block.name))
    plant.connections = plant.connections?.filter(edge => {
      const [from,to] = edgeEndpoints(edge)
      return !addedBlocks.has(from.split('.')[0]) && !addedBlocks.has(to.split('.')[0]) && !commands.has(to)
    })
    plant.notes = 'Keep my plant note'
    plant.blocks!.find(block=>block.name==='one')!.parameters![0].value = 0.9
    const saved: Project = {version:1,name:old.name,graph:old,scenario:{...defaultScenario},layout:{plant:{x:150,y:200}}}
    const migrated = migrateWaterLoopControl(saved,reference)
    const updated = migrated.graph.blocks?.find(block=>block.name==='plant')!
    assert.equal(updated.notes,'Keep my plant note')
    assert.equal(updated.blocks?.find(block=>block.name==='one')?.parameters?.[0].value,0.9)
    assert.ok(updated.blocks?.some(block=>block.name==='hot_water_pump_select'))
    assert.ok(updated.blocks?.some(block=>block.name==='chilled_water_pump_select'))
    assert.equal(updated.outputs?.find(port=>port.name==='hot_water_enable')?.type,'Boolean')
    assert.equal(migrated.graph.outputs?.find(port=>port.name==='plant_chilled_water_enable')?.type,'Boolean')
    for(const name of sensors){
      assert.equal(migrated.graph.inputs?.filter(port=>port.name===name).length,1)
      assert.equal(migrated.graph.connections?.filter(edge=>edgeEndpoints(edge).join('|')===`${name}|plant.${name}`).length,1)
    }
    for(const name of commands){
      assert.equal(updated.connections?.filter(edge=>edgeEndpoints(edge)[1]===name).length,1)
      assert.equal(migrated.graph.outputs?.filter(port=>port.name===`plant_${name}`).length,1)
    }
    assert.deepEqual(migrated.layout.plant,{x:150,y:200})
    assert.equal(migrateWaterLoopControl(migrated,reference),migrated)
  })
}
