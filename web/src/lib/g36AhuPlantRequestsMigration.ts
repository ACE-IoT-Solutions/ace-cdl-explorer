import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add the separate OBC AHU plant requests without replacing saved control tuning. */
export function migrateG36AhuPlantRequests(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const plant = saved.graph.blocks?.find(block => block.name === 'plant')
  if (plant?.blocks?.some(block => block.name === 'ahu_hot_plant_request_count')) return saved
  const source = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !source) throw new Error('AHU plant request migration requires the G36 plant')
  const next = structuredClone(saved)
  const target = next.graph.blocks!.find(block => block.name === 'plant')!
  const newBlock = (name: string) => /^ahu_(?:1|2)_(?:hot|chilled)_plant_/.test(name) ||
    /^ahu_(?:hot|chilled)_plant_/.test(name)
  const addEdge = (block: Block, from: string, to: string) => {
    if (!(block.connections ?? []).some(edge => {
      const [a, b] = edgeEndpoints(edge)
      return a === from && b === to
    })) (block.connections ??= []).push({ from, to })
  }
  for (const block of source.blocks ?? []) {
    if (newBlock(block.name)) (target.blocks ??= []).push(structuredClone(block))
  }
  for (const direction of ['outputs'] as const) {
    for (const port of source[direction] ?? []) {
      if (newBlock(port.name) && !target[direction]?.some(existing => existing.name === port.name))
        (target[direction] ??= []).push(structuredClone(port))
    }
    for (const port of reference[direction] ?? []) {
      if ((newBlock(port.name) || /^plant_ahu_(?:hot|chilled)_plant_request_count_total$/.test(port.name)) &&
          !next.graph[direction]?.some(existing => existing.name === port.name))
        (next.graph[direction] ??= []).push(structuredClone(port))
    }
  }
  const oldEdges = new Set([
    'hot_plant_request_or_heat.y→freeze_hot_water_pump_enable.u1',
    'chilled_water_pump_needed.y→freeze_chilled_water_pump_enable.u1',
  ])
  target.connections = target.connections?.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !oldEdges.has(`${from}→${to}`)
  })
  for (const edge of source.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (newBlock(from.split('.')[0]) || newBlock(to.split('.')[0]) || newBlock(to)) addEdge(target, from, to)
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (newBlock(from.replace(/^plant\./, '')) || newBlock(to.replace(/^plant\./, '')))
      addEdge(next.graph, from, to)
  }
  for (const block of target.blocks ?? []) {
    if (!/^ahu_(?:1|2)_(?:hot_)?(?:two|three)_request_delay$/.test(block.name)) continue
    const parameter = block.parameters?.find(parameter => parameter.name === 'delayOnInit')
    if (parameter) parameter.value = false
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
