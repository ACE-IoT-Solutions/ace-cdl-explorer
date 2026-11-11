import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const hasEdge = (block: Block, from: string, to: string) =>
  block.connections?.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`) ?? false

export function needsG36WaterCapacityMigration(saved: Project): boolean {
  const plant = saved.graph.blocks?.find(block => block.name === 'plant')
  if (!plant?.blocks?.some(block => block.name === 'heat_pump_allocated_fraction') ||
      !saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return false
  return ['heating', 'cooling'].some(service => {
    const capacity = `heat_pump_${service}_available_capacity_kw`
    const limiter = `heat_pump_${service}_service_cap`
    return !saved.graph.inputs?.some(port => port.name === capacity) ||
      !plant.inputs?.some(port => port.name === capacity) ||
      !plant.blocks?.some(block => block.name === limiter) ||
      !hasEdge(saved.graph, capacity, `plant.${capacity}`) ||
      !hasEdge(plant, `heat_pump_${service}.y`, `${limiter}.u1`) ||
      !hasEdge(plant, capacity, `${limiter}.u2`)
  })
}

/** Feed measured service capacities into an existing editable G36 plant. */
export function migrateG36WaterCapacity(saved: Project, reference: Block): Project {
  if (!needsG36WaterCapacityMigration(saved)) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks!.find(block => block.name === 'plant')!
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!template) throw new Error('G36 capacity migration requires a plant reference')
  for (const service of ['heating', 'cooling'] as const) {
    const capacity = `heat_pump_${service}_available_capacity_kw`
    const limiter = `heat_pump_${service}_service_cap`
    const raw = `heat_pump_${service}.y`
    if (!next.graph.inputs?.some(port => port.name === capacity)) {
      next.graph.inputs = [...(next.graph.inputs ?? []), structuredClone(reference.inputs!.find(port => port.name === capacity)!)]
    }
    if (!plant.inputs?.some(port => port.name === capacity)) {
      plant.inputs = [...(plant.inputs ?? []), structuredClone(template.inputs!.find(port => port.name === capacity)!)]
    }
    next.graph.connections ??= []
    if (!next.graph.connections.some(edge => edgeEndpoints(edge).join('→') === `${capacity}→plant.${capacity}`)) {
      next.graph.connections.push({from: capacity, to: `plant.${capacity}`})
    }
    plant.blocks ??= []
    if (!plant.blocks.some(block => block.name === limiter)) {
      plant.blocks.push(structuredClone(template.blocks!.find(block => block.name === limiter)!))
    }
    plant.connections ??= []
    plant.connections = plant.connections.map(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === raw && to !== `${limiter}.u1` ? {from: `${limiter}.y`, to} : edge
    })
    for (const [from, to] of [[raw, `${limiter}.u1`], [capacity, `${limiter}.u2`]]) {
      if (!plant.connections.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
        plant.connections.push({from, to})
      }
    }
  }
  next.layout = {...layoutFor(next.graph), ...saved.layout}
  return next
}
