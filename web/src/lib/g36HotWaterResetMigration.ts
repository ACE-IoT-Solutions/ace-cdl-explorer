import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add the sampled hot-water reset while retaining saved plant edits. */
export function migrateG36HotWaterReset(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.find(block => block.name === 'plant')?.blocks?.some(block => block.name === 'hot_reset_previous')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const plant = graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 hot-water reset migration requires the plant composite')
  const addPort = (target: Block, source: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = source[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`Current G36 example lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => { const [a, b] = edgeEndpoints(edge); return a === from && b === to })) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (const index of [1, 2]) addPort(plant, template, 'inputs', `ahu_${index}_heating_valve`)
  addPort(plant, template, 'outputs', 'hot_water_reset_fraction')
  addPort(graph, reference, 'outputs', 'plant_hot_water_reset_fraction')
  const added = new Set((template.blocks ?? []).map(block => block.name).filter(name =>
    name.startsWith('hot_reset_') || /^ahu_[12]_hot_/.test(name)
  ))
  for (const child of template.blocks ?? []) {
    if (added.has(child.name) && !plant.blocks?.some(block => block.name === child.name)) {
      (plant.blocks ??= []).push(structuredClone(child))
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !(from === 'hot_water_supply_setpoint.y' &&
      (to === 'hot_water_supply_setpoint_c' || to === 'hot_water_source_error.u1'))
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (added.has(from.split('.')[0]) || added.has(to.split('.')[0]) || to === 'hot_water_reset_fraction') {
      addEdge(plant, from, to)
    }
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (to === 'plant.ahu_1_heating_valve' || to === 'plant.ahu_2_heating_valve' || to === 'plant_hot_water_reset_fraction') {
      addEdge(graph, from, to)
    }
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
