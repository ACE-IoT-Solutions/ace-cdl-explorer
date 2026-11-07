import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const names = new Set([
  'co2_reset_lower', 'co2_reset_span', 'co2_above_lower', 'co2_reset_ratio',
  'co2_reset_limit', 'co2_cooling_max_flow', 'co2_available_flow',
  'co2_flow_increment', 'co2_flow_reset', 'co2_minimum_flow',
])
const zoneName = (index: number) => `zone_${String(index).padStart(2, '0')}`
const wireId = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')

export function needsG36Co2Migration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    Array.from({ length: 10 }, (_, index) => zoneName(index + 1)).some(name =>
      !project.graph.inputs?.some(port => port.name === `${name}_co2_ppm`) ||
      !project.graph.blocks?.find(block => block.name === name)?.blocks?.some(block => block.name === 'co2_minimum_flow'))
}

/** Add measured CO₂ and editable occupied-flow reset without replacing saved tuning. */
export function migrateG36Co2(project: Project, reference: Block): Project {
  if (!needsG36Co2Migration(project)) return project
  const next = structuredClone(project)
  const graph = next.graph
  const addPort = (target: Block, template: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = template[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Missing G36 CO₂ reference port ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, edge: NonNullable<Block['connections']>[number]) => {
    if (!target.connections?.some(existing => wireId(existing) === wireId(edge)))
      (target.connections ??= []).push(structuredClone(edge))
  }
  for (let index = 1; index <= 10; index++) {
    const name = zoneName(index)
    const zone = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`Missing G36 CO₂ zone ${name}`)
    addPort(graph, reference, 'inputs', `${name}_co2_ppm`)
    addPort(zone, template, 'inputs', 'co2_ppm')
    for (const block of template.blocks ?? []) {
      if (names.has(block.name) && !zone.blocks?.some(child => child.name === block.name))
        (zone.blocks ??= []).push(structuredClone(block))
    }
    zone.connections = zone.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !(from === 'minimum_airflow_heating.y' &&
        ['airflow_sum.u1', 'airflow_request_sum.u1'].includes(to))
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (names.has(from.split('.')[0]) || names.has(to.split('.')[0])) addEdge(zone, edge)
    }
    for (const edge of reference.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from === `${name}_co2_ppm` && to === `${name}.co2_ppm`) addEdge(graph, edge)
    }
    const entity = reference.brick_model?.entities.find(item => item.id === `${name}_co2_ppm`)
    if (entity && graph.brick_model && !graph.brick_model.entities.some(item => item.id === entity.id)) {
      graph.brick_model.entities.push(structuredClone(entity))
      const relationship = reference.brick_model?.relationships.find(item =>
        item.object === entity.id && item.predicate === 'brick:hasPoint')
      if (relationship) graph.brick_model.relationships.push(structuredClone(relationship))
    }
  }
  next.layout = { ...layoutFor(graph), ...project.layout }
  return next
}
