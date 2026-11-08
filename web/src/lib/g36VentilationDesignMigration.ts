import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const designBlocks = new Set([
  'ventilation_floor_area', 'ventilation_area_rate', 'ventilation_area_design',
  'ventilation_person_rate', 'ventilation_population_design',
])
const correctionBlocks = new Set(['ventilation_window_eligible', 'co2_population_component'])

/** Expose zone design area and per-person rates without discarding saved flow tuning. */
export function migrateG36VentilationDesign(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if ((saved.graph.blocks ?? []).filter(block => block.block_type === 'VAVBoxController')
    .every(block => block.blocks?.some(child => child.name === 'ventilation_floor_area')
      && block.blocks?.some(child => child.name === 'ventilation_window_eligible')
      && block.blocks?.some(child => child.name === 'co2_population_component')
      && block.inputs?.some(port => port.name === 'occupancy_count')
      && (!saved.graph.brick_model || saved.graph.brick_model.entities.some(entity => entity.id === `${block.name}_occupancy_count`)))) return saved

  const next = structuredClone(saved)
  const graph = next.graph
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 ventilation design migration requires ${name}`)
    const hasDesign = zone.blocks?.some(block => block.name === 'ventilation_floor_area')
    let areaFlow: unknown
    let populationFlow: unknown
    if (!hasDesign) {
      const oldArea = zone.blocks?.find(block => block.name === 'ventilation_area_design')
      const oldPopulation = zone.blocks?.find(block => block.name === 'ventilation_population_design')
      if (oldArea?.block_type !== 'Constant' || oldPopulation?.block_type !== 'Constant') continue
      areaFlow = oldArea.parameters?.find(parameter => parameter.name === 'k')?.value
      populationFlow = oldPopulation.parameters?.find(parameter => parameter.name === 'k')?.value
      zone.blocks = (zone.blocks ?? []).filter(block => !designBlocks.has(block.name))
    }
    const occupants = saved.scenario?.design_occupants_per_zone || 8
    for (const child of template.blocks ?? []) {
      if (!(designBlocks.has(child.name) && !hasDesign || correctionBlocks.has(child.name))) continue
      if (zone.blocks?.some(existing => existing.name === child.name)) continue
      const added = structuredClone(child)
      const rate = added.parameters?.find(parameter => parameter.name === 'k')
      if (rate && added.name === 'ventilation_area_rate' && typeof areaFlow === 'number') rate.value = areaFlow / 100
      if (rate && added.name === 'ventilation_person_rate' && typeof populationFlow === 'number') rate.value = populationFlow / occupants
      ;(zone.blocks ??= []).push(added)
    }
    const countPort = template.inputs?.find(port => port.name === 'occupancy_count')
    const rootPort = reference.inputs?.find(port => port.name === `${name}_occupancy_count`)
    if (!countPort || !rootPort) throw new Error(`G36 ventilation design reference lacks ${name} count`)
    if (!zone.inputs?.some(port => port.name === 'occupancy_count')) (zone.inputs ??= []).push(structuredClone(countPort))
    if (!graph.inputs?.some(port => port.name === rootPort.name)) (graph.inputs ??= []).push(structuredClone(rootPort))
    addEdge(graph, rootPort.name, `${name}.occupancy_count`)
    const entity = reference.brick_model?.entities.find(item => item.id === rootPort.name)
    if (entity && graph.brick_model && !graph.brick_model.entities.some(item => item.id === entity.id)) {
      graph.brick_model.entities.push(structuredClone(entity))
      const relationship = reference.brick_model?.relationships.find(item => item.object === entity.id && item.predicate === 'brick:hasPoint')
      if (relationship) graph.brick_model.relationships.push(structuredClone(relationship))
    }
    zone.connections = zone.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !(to === 'ventilation_area_active.u3' && from === 'ventilation_occupied.y'
        || to === 'ventilation_population_active.u3' && from === 'ventilation_occupied.y'
        || to === 'ventilation_population_active.u1' && from === 'ventilation_population_design.y')
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (designBlocks.has(from.split('.')[0]) || designBlocks.has(to.split('.')[0])
        || correctionBlocks.has(from.split('.')[0]) || correctionBlocks.has(to.split('.')[0])
        || to === 'ventilation_area_active.u3' || to === 'ventilation_population_active.u3'
        || to === 'ventilation_population_active.u1') addEdge(zone, from, to)
    }
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
