import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const zoneBlocks = new Set([
  'ventilation_occupied', 'ventilation_window_eligible', 'ventilation_floor_area', 'ventilation_area_rate',
  'ventilation_area_design', 'ventilation_person_rate', 'ventilation_population_design',
  'ventilation_zero', 'ventilation_area_active', 'ventilation_population_active',
  'ventilation_breathing_sum', 'ventilation_cooling_supply',
  'ventilation_cooling_effectiveness', 'ventilation_heating_effectiveness',
  'ventilation_effectiveness', 'ventilation_minimum_oa',
])
const zoneOutputs = ['ventilation_area', 'ventilation_population', 'ventilation_minimum_oa_flow']
const ahuOutputs = ['ventilation_uncorrected', 'ventilation_efficiency']
const ventilationBlock = (name: string) => name.startsWith('vent_') || name.includes('_vent_')

/** Replace the teaching flow multiplier with zone and AHU ventilation calculations. */
export function migrateG36Ventilation(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.filter(block => block.block_type === 'AHUController')
    .every(block => block.blocks?.some(child => child.name === 'vent_effective_limit'))
    && saved.graph.blocks?.filter(block => block.block_type === 'VAVBoxController')
      .every(block => block.blocks?.some(child => child.name === 'ventilation_minimum_oa'))) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const addPort = (target: Block, template: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = template[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`G36 ventilation reference lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 ventilation migration requires ${name}`)
    for (const blockName of ['standby_damper', 'standby_airflow']) {
      const value = zone.blocks?.find(block => block.name === blockName)?.parameters?.find(parameter => parameter.name === 'k')
      if (value?.value === 0.05) value.value = 0
    }
    for (const output of zoneOutputs) {
      addPort(zone, template, 'outputs', output)
      addPort(graph, reference, 'outputs', `${name}_${output}`)
    }
    addPort(zone, template, 'inputs', 'occupancy_count')
    addPort(graph, reference, 'inputs', `${name}_occupancy_count`)
    addEdge(graph, `${name}_occupancy_count`, `${name}.occupancy_count`)
    for (const child of template.blocks ?? []) {
      if (zoneBlocks.has(child.name) && !zone.blocks?.some(block => block.name === child.name)) {
        (zone.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (zoneBlocks.has(from.split('.')[0]) || zoneBlocks.has(to.split('.')[0])
        || zoneOutputs.includes(to)) addEdge(zone, from, to)
    }
  }
  for (let index = 1; index <= 2; index++) {
    const name = `ahu_${index}`
    const ahu = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 ventilation migration requires ${name}`)
    for (const input of template.inputs ?? []) {
      if (input.name.includes('_ventilation_') || input.name.endsWith('_measured_airflow')) {
        addPort(ahu, template, 'inputs', input.name)
      }
    }
    for (const output of ahuOutputs) {
      addPort(ahu, template, 'outputs', output)
      addPort(graph, reference, 'outputs', `${name}_${output}`)
    }
    ahu.blocks = ahu.blocks?.filter(block => block.name !== 'minimum_oa_flow_gain')
    ahu.connections = ahu.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from.split('.')[0] !== 'minimum_oa_flow_gain' && to.split('.')[0] !== 'minimum_oa_flow_gain'
    })
    for (const child of template.blocks ?? []) {
      if (ventilationBlock(child.name) && !ahu.blocks?.some(block => block.name === child.name)) {
        (ahu.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (ventilationBlock(from.split('.')[0]) || ventilationBlock(to.split('.')[0])
        || ahuOutputs.includes(to) || to === 'minimum_oa_setpoint.u1') addEdge(ahu, from, to)
    }
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (from.includes('_ventilation_') || to.includes('_ventilation_')
      || from.includes('.ventilation_') || to.includes('.ventilation_')
      || (to.startsWith('ahu_') && to.endsWith('_measured_airflow'))) addEdge(graph, from, to)
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
