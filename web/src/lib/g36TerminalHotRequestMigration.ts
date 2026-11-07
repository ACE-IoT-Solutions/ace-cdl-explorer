import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Connect saved G36 VAV reheat commands to the editable hot-water plant reset. */
export function migrateG36TerminalHotRequests(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.find(block => block.name === 'plant')?.blocks?.some(block => block.name === 'hot_reset_all_request_count')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const plant = graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 terminal hot-water migration requires the plant composite')
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
  for (let index = 1; index <= 10; index++) {
    const zone = `zone_${String(index).padStart(2, '0')}`
    for (const suffix of ['reheat_valve', 'heating_demand']) addPort(plant, template, 'inputs', `${zone}_${suffix}`)
  }
  addPort(plant, template, 'outputs', 'terminal_hot_water_request_count')
  addPort(graph, reference, 'outputs', 'plant_terminal_hot_water_request_count')
  const added = new Set((template.blocks ?? []).map(block => block.name).filter(name =>
    name.startsWith('terminal_') || /^zone_\d\d_hot_/.test(name) || name === 'hot_reset_all_request_count'
  ))
  for (const child of template.blocks ?? []) {
    if (added.has(child.name) && !plant.blocks?.some(block => block.name === child.name)) {
      (plant.blocks ??= []).push(structuredClone(child))
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !(from === 'hot_reset_request_count.y' && to === 'hot_reset_request_real.u')
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (added.has(from.split('.')[0]) || added.has(to.split('.')[0]) || to === 'terminal_hot_water_request_count') {
      addEdge(plant, from, to)
    }
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (/^plant.zone_\d\d_(reheat_valve|heating_demand)$/.test(to) || to === 'plant_terminal_hot_water_request_count') {
      addEdge(graph, from, to)
    }
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
