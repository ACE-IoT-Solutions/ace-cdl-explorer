import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const oldNames = new Set(['heat_pump_dispatch_fraction', 'heat_pump_dispatch_limit', 'backup_fraction'])
const newNames = new Set([
  'heat_pump_hot_eligible', 'heat_pump_chilled_eligible',
  'heat_pump_utilization_limit', 'heat_pump_zero_capacity',
  'heat_pump_capacity_nonnegative', 'heat_pump_capacity_limited', 'heat_pump_total_load', 'heat_pump_divisor_floor',
  'heat_pump_safe_total', 'heat_pump_capacity_ratio', 'heat_pump_full_fraction',
  'heat_pump_allocated_fraction', 'boiler_residual', 'chiller_residual',
  'heat_pump_mode_threshold', 'heat_pump_heating_requested', 'heat_pump_cooling_requested',
  'heat_pump_heating_mode_code', 'heat_pump_cooling_mode_code',
  'heat_pump_mode_cooling_weight', 'heat_pump_cooling_mode_weight', 'heat_pump_mode_sum',
])
const dispatchTargets = new Set([
  'heat_pump_heating.u1', 'heat_pump_heating.u2',
  'heat_pump_cooling.u1', 'heat_pump_cooling.u2',
  'boiler.u1', 'boiler.u2', 'chiller.u1', 'chiller.u2',
])

/** Replace a fixed G36 source split with capacity-aware CDL allocation. */
export function migrateG36HybridDispatch(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const previous = saved.graph.blocks?.find(block => block.name === 'plant')
  if (previous?.blocks?.some(block => block.name === 'heat_pump_allocated_fraction')) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 hybrid dispatch migration requires plant')
  const oldShare = plant.blocks?.find(block => block.name === 'heat_pump_dispatch_fraction')
    ?.parameters?.find(parameter => parameter.name === 'k')?.value
  plant.blocks = (plant.blocks ?? []).filter(block => !oldNames.has(block.name))
  for (const block of template.blocks ?? []) {
    if (!newNames.has(block.name) || plant.blocks.some(existing => existing.name === block.name)) continue
    const added = structuredClone(block)
    if (added.name === 'heat_pump_utilization_limit' && typeof oldShare === 'number') {
      const parameter = added.parameters?.find(parameter => parameter.name === 'k')
      if (parameter) parameter.value = Math.max(0, Math.min(1, oldShare))
    }
    plant.blocks.push(added)
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !oldNames.has(from.split('.')[0]) && !oldNames.has(to.split('.')[0])
      && !dispatchTargets.has(to)
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!newNames.has(from.split('.')[0]) && !newNames.has(to.split('.')[0])
      && !dispatchTargets.has(to) && to !== 'heat_pump_requested_mode') continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  for (const name of ['heat_pump_available_capacity_kw']) {
    if (!next.graph.inputs?.some(port => port.name === name)) {
      next.graph.inputs = [...(next.graph.inputs ?? []), structuredClone(reference.inputs!.find(port => port.name === name)!)]
    }
    if (!plant.inputs?.some(port => port.name === name)) {
      plant.inputs = [...(plant.inputs ?? []), structuredClone(template.inputs!.find(port => port.name === name)!)]
    }
    if (!next.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === `${name}→plant.${name}`)) {
      next.graph.connections = [...(next.graph.connections ?? []), { from: name, to: `plant.${name}` }]
    }
  }
  if (!plant.outputs?.some(port => port.name === 'heat_pump_requested_mode')) {
    plant.outputs = [...(plant.outputs ?? []), structuredClone(template.outputs!.find(port => port.name === 'heat_pump_requested_mode')!)]
  }
  if (!next.graph.outputs?.some(port => port.name === 'plant_heat_pump_requested_mode')) {
    next.graph.outputs = [...(next.graph.outputs ?? []), structuredClone(reference.outputs!.find(port => port.name === 'plant_heat_pump_requested_mode')!)]
  }
  if (!next.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === 'plant.heat_pump_requested_mode→plant_heat_pump_requested_mode')) {
    next.graph.connections = [...(next.graph.connections ?? []),
      { from: 'plant.heat_pump_requested_mode', to: 'plant_heat_pump_requested_mode' }]
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
