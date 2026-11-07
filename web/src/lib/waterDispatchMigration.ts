import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const dispatchTargets = new Set([
  'backup_fraction.u2', 'heat_pump_heating.u2', 'heat_pump_cooling.u2',
])

/** Carry the old scenario share into the editable plant CDL dispatch constant. */
export function migrateWaterDispatch(saved: Project, reference: Block): Project {
  if (!['FourPipeBuildingControls', 'FourPipeG36Controls'].includes(saved.graph.name)) return saved
  const oldPlant = saved.graph.blocks?.find(block => block.name === 'plant')
  if (saved.graph.name === 'FourPipeG36Controls' && oldPlant?.blocks?.some(block => block.name === 'heat_pump_allocated_fraction')) {
    if (saved.scenario.heat_pump_fraction === undefined && !saved.graph.inputs?.some(port => port.name === 'heat_pump_fraction')) return saved
    const next = structuredClone(saved)
    delete next.scenario.heat_pump_fraction
    next.graph.inputs = next.graph.inputs?.filter(port => port.name !== 'heat_pump_fraction')
    next.graph.connections = next.graph.connections?.filter(edge =>
      !edgeEndpoints(edge).some(endpoint => endpoint === 'heat_pump_fraction' || endpoint === 'plant.heat_pump_fraction'))
    const plant = next.graph.blocks?.find(block => block.name === 'plant')
    if (plant) plant.inputs = plant.inputs?.filter(port => port.name !== 'heat_pump_fraction')
    return next
  }
  const hasConstant = oldPlant?.blocks?.some(block => block.name === 'heat_pump_dispatch_fraction')
  if (hasConstant && !saved.graph.inputs?.some(port => port.name === 'heat_pump_fraction')) {
    if (saved.scenario.heat_pump_fraction === undefined) return saved
    const next = structuredClone(saved)
    delete next.scenario.heat_pump_fraction
    return next
  }
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('Water dispatch migration requires plant')
  const oldFraction = next.scenario.heat_pump_fraction
  delete next.scenario.heat_pump_fraction
  next.graph.inputs = next.graph.inputs?.filter(port => port.name !== 'heat_pump_fraction')
  next.graph.connections = next.graph.connections?.filter(edge =>
    !edgeEndpoints(edge).some(endpoint => endpoint === 'heat_pump_fraction' || endpoint === 'plant.heat_pump_fraction'))
  plant.inputs = plant.inputs?.filter(port => port.name !== 'heat_pump_fraction')
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return from !== 'heat_pump_fraction' && !dispatchTargets.has(to)
  })
  for (const name of ['heat_pump_dispatch_fraction', 'heat_pump_dispatch_limit']) {
    if (plant.blocks?.some(block => block.name === name)) continue
    const source = template.blocks?.find(block => block.name === name)
    if (!source) throw new Error(`Water dispatch reference lacks ${name}`)
    if (!plant.blocks) plant.blocks = []
    const added = structuredClone(source)
    if (name === 'heat_pump_dispatch_fraction' && typeof oldFraction === 'number') {
      const parameter = added.parameters?.find(parameter => parameter.name === 'k')
      if (parameter) parameter.value = Math.max(0, Math.min(1, oldFraction))
    }
    plant.blocks.push(added)
  }
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!from.startsWith('heat_pump_dispatch_') && !to.startsWith('heat_pump_dispatch_')) continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
