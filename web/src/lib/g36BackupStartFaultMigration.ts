import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const backupNames = ['boiler', 'chiller'] as const
const suffixes = [
  'start_request_observed', 'start_threshold', 'start_requested', 'not_running',
  'start_unproven', 'start_available', 'start_timeout', 'start_fault_latch',
  'fault_clear', 'can_dispatch',
]
const added = new Set(backupNames.flatMap(name => suffixes.map(suffix => `${name}_${suffix}`)))
const gateTargets = new Set(backupNames.map(name => `${name}_availability_gate.u3`))

/** Add backup-source fault diagnosis while retaining the saved plant's tuning. */
export function migrateG36BackupStartFault(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const previous = saved.graph.blocks?.find(block => block.name === 'plant')
  if (previous?.blocks?.some(block => block.name === 'boiler_start_fault_latch')) return saved
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!previous || !template) throw new Error('G36 backup start fault migration requires plant')
  const next = structuredClone(saved)
  const plant = next.graph.blocks!.find(block => block.name === 'plant')!
  for (const name of backupNames) {
    const reset = `${name}_fault_reset`
    const fault = `${name}_start_fault`
    next.graph.inputs = [...(next.graph.inputs ?? []), structuredClone(reference.inputs!.find(port => port.name === reset)!)]
    plant.inputs = [...(plant.inputs ?? []), structuredClone(template.inputs!.find(port => port.name === reset)!)]
    next.graph.outputs = [...(next.graph.outputs ?? []), structuredClone(reference.outputs!.find(port => port.name === `plant_${fault}`)!)]
    plant.outputs = [...(plant.outputs ?? []), structuredClone(template.outputs!.find(port => port.name === fault)!)]
  }
  for (const block of template.blocks ?? []) {
    if (added.has(block.name)) plant.blocks = [...(plant.blocks ?? []), structuredClone(block)]
  }
  plant.connections = (plant.connections ?? []).filter(edge => !gateTargets.has(edgeEndpoints(edge)[1]))
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0]) &&
      !backupNames.some(name => to === `${name}_start_fault`)) continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  for (const edge of reference.connections ?? []) {
    const [, to] = edgeEndpoints(edge)
    if (backupNames.some(name => to === `plant.${name}_fault_reset` || to === `plant_${name}_start_fault`)) {
      next.graph.connections = [...(next.graph.connections ?? []), structuredClone(edge)]
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
