import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = new Set([
  'heat_pump_start_request', 'heat_pump_start_request_observed', 'heat_pump_start_threshold',
  'heat_pump_start_requested', 'heat_pump_not_running', 'heat_pump_start_unproven',
  'heat_pump_start_available', 'heat_pump_start_timeout', 'heat_pump_start_fault_latch',
  'heat_pump_fault_clear', 'heat_pump_can_dispatch',
])

/** Add source failure diagnosis without replacing any saved dispatch tuning. */
export function migrateG36StartFault(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const previous = saved.graph.blocks?.find(block => block.name === 'plant')
  if (previous?.blocks?.some(block => block.name === 'heat_pump_start_fault_latch')) return saved
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!previous || !template) throw new Error('G36 start fault migration requires plant')
  const next = structuredClone(saved)
  const plant = next.graph.blocks!.find(block => block.name === 'plant')!
  for (const name of ['heat_pump_fault_reset']) {
    next.graph.inputs = [...(next.graph.inputs ?? []), structuredClone(reference.inputs!.find(port => port.name === name)!)]
    plant.inputs = [...(plant.inputs ?? []), structuredClone(template.inputs!.find(port => port.name === name)!)]
  }
  next.graph.outputs = [...(next.graph.outputs ?? []), structuredClone(reference.outputs!.find(port => port.name === 'plant_heat_pump_start_fault')!)]
  plant.outputs = [...(plant.outputs ?? []), structuredClone(template.outputs!.find(port => port.name === 'heat_pump_start_fault')!)]
  for (const block of template.blocks ?? []) {
    if (added.has(block.name)) plant.blocks = [...(plant.blocks ?? []), structuredClone(block)]
  }
  plant.connections = (plant.connections ?? []).filter(edge => edgeEndpoints(edge)[1] !== 'heat_pump_availability_gate.u3')
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0]) && to !== 'heat_pump_start_fault') continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) plant.connections.push(structuredClone(edge))
  }
  for (const edge of reference.connections ?? []) {
    const [, to] = edgeEndpoints(edge)
    if (to === 'plant.heat_pump_fault_reset' || to === 'plant_heat_pump_start_fault') {
      next.graph.connections = [...(next.graph.connections ?? []), structuredClone(edge)]
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
