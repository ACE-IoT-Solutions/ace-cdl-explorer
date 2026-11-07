import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const statusNames = ['heat_pump_available', 'boiler_available', 'chiller_available']
const gateNames = new Set(['heat_pump_availability_gate', 'boiler_availability_gate', 'chiller_availability_gate'])
const replacedTargets = new Set([
  'heat_pump_capacity_ratio.u1', 'boiler_kw', 'chiller_kw',
  'hot_stage_backup_active.u1', 'chilled_stage_backup_active.u1',
  'heat_pump_heating_requested.u1', 'heat_pump_cooling_requested.u1',
])

/** Add raw equipment status and CDL source gates to saved G36 projects. */
export function migrateG36EquipmentAvailability(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const oldPlant = saved.graph.blocks?.find(block => block.name === 'plant')
  if (oldPlant?.blocks?.some(block => block.name === 'heat_pump_availability_gate')) return saved
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!oldPlant || !template) throw new Error('G36 equipment availability migration requires plant')
  const next = structuredClone(saved)
  const plant = next.graph.blocks!.find(block => block.name === 'plant')!
  for (const name of statusNames) {
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
  for (const block of template.blocks ?? []) {
    if (gateNames.has(block.name) && !plant.blocks?.some(existing => existing.name === block.name)) {
      plant.blocks = [...(plant.blocks ?? []), structuredClone(block)]
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => !replacedTargets.has(edgeEndpoints(edge)[1]))
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!gateNames.has(from.split('.')[0]) && !gateNames.has(to.split('.')[0]) && !replacedTargets.has(to)) continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
