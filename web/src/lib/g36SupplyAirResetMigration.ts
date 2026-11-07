import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add VAV-requested AHU supply-air reset without replacing saved AHU parameters. */
export function migrateG36SupplyAirReset(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.find(block => block.name === 'ahu_1')?.blocks?.some(block => block.name === 'sat_previous')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 supply-air reset migration requires ${name}`)
    const addPort = (target: Block, source: Block, direction: 'inputs' | 'outputs', portName: string) => {
      const port = source[direction]?.find(item => item.name === portName)
      if (!port) throw new Error(`Current G36 example lacks ${portName}`)
      if (!target[direction]?.some(item => item.name === portName)) (target[direction] ??= []).push(structuredClone(port))
    }
    const addEdge = (target: Block, from: string, to: string) => {
      if (!(target.connections ?? []).some(edge => { const [a, b] = edgeEndpoints(edge); return a === from && b === to })) {
        (target.connections ??= []).push({ from, to })
      }
    }
    const added = new Set((template.blocks ?? []).map(block => block.name).filter(blockName =>
      blockName.startsWith('sat_') || /^zone_\d\d_sat_/.test(blockName)
    ))
    for (const child of template.blocks ?? []) {
      if (added.has(child.name) && !ahu.blocks?.some(block => block.name === child.name)) {
        (ahu.blocks ??= []).push(structuredClone(child))
      }
    }
    for (let zoneIndex = (index - 1) * 5 + 1; zoneIndex <= index * 5; zoneIndex++) {
      const zone = `zone_${String(zoneIndex).padStart(2, '0')}`
      for (const signal of ['cooling_demand', 'damper_position']) addPort(ahu, template, 'inputs', `${zone}_${signal}`)
    }
    addPort(ahu, template, 'outputs', 'sat_request_count')
    addPort(graph, reference, 'outputs', `${name}_sat_request_count`)
    const replacedTargets = new Set((template.connections ?? [])
      .filter(edge => edgeEndpoints(edge)[0] === 'sat_output.y')
      .map(edge => edgeEndpoints(edge)[1]))
    ahu.connections = (ahu.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !(from === 'supply_reset.y' && replacedTargets.has(to))
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
      if (from === 'sat_output.y' && ahu.connections.some(existing => edgeEndpoints(existing)[1] === to)) continue
      addEdge(ahu, from, to)
    }
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (/^ahu_[12]\.zone_\d\d_(cooling_demand|damper_position)$/.test(to) || /^ahu_[12]_sat_request_count$/.test(to)) {
      if (!(graph.connections ?? []).some(existing => { const [a, b] = edgeEndpoints(existing); return a === from && b === to })) {
        (graph.connections ??= []).push({ from, to })
      }
    }
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
