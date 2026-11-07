import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const sensorNames = [
  'hot_water_supply_temp_c', 'hot_water_return_temp_c',
  'chilled_water_supply_temp_c', 'chilled_water_return_temp_c',
  'hot_water_flow_kg_s', 'chilled_water_flow_kg_s',
]
const commandNames = [
  'hot_water_enable', 'chilled_water_enable',
  'hot_water_pump_speed', 'chilled_water_pump_speed',
  'hot_water_supply_setpoint_c', 'chilled_water_supply_setpoint_c',
]
const newBlockNames = new Set([
  'pump_zero', 'pump_start_threshold',
  'hot_water_pump_needed', 'chilled_water_pump_needed',
  'hot_water_pump_gain', 'chilled_water_pump_gain',
  'hot_water_pump_limit', 'chilled_water_pump_limit',
  'hot_water_pump_select', 'chilled_water_pump_select',
  'hot_water_supply_setpoint', 'chilled_water_supply_setpoint',
])

/** Add physical water points and graph-owned plant pump/temperature commands to saved buildings. */
export function migrateWaterLoopControl(saved: Project, reference: Block): Project {
  if (!['FourPipeBuildingControls', 'FourPipeG36Controls'].includes(saved.graph.name)) return saved
  if (commandNames.every(name => saved.graph.outputs?.some(port => port.name === `plant_${name}`))
    && sensorNames.every(name => saved.graph.inputs?.some(port => port.name === name))) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('Water-loop migration requires the plant composite')
  const copyPort = (target: Block, source: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current building example lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const connect = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('|') === `${from}|${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (const name of sensorNames) {
    copyPort(next.graph, reference, 'inputs', name)
    copyPort(plant, template, 'inputs', name)
    connect(next.graph, name, `plant.${name}`)
  }
  for (const name of commandNames) {
    copyPort(next.graph, reference, 'outputs', `plant_${name}`)
    copyPort(plant, template, 'outputs', name)
    connect(next.graph, `plant.${name}`, `plant_${name}`)
  }
  for (const block of template.blocks ?? []) {
    if (newBlockNames.has(block.name) && !plant.blocks?.some(item => item.name === block.name)) {
      (plant.blocks ??= []).push(structuredClone(block))
    }
  }
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (newBlockNames.has(from.split('.')[0]) || newBlockNames.has(to.split('.')[0])
      || commandNames.includes(to)) connect(plant, from, to)
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
