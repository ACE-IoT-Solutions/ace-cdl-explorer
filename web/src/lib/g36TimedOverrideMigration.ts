import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const identity = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')

export function needsG36TimedOverrideMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' &&
    !!saved.graph.inputs?.some(port => port.name === 'zone_01_override') &&
    Array.from({ length: 10 }, (_, index) => `zone_${String(index + 1).padStart(2, '0')}_override_hold`)
      .some(name => !saved.graph.blocks?.some(block => block.name === name))
}

/** Time each raw local button press in CDL before it affects zone or group occupancy. */
export function migrateG36TimedOverrides(saved: Project, reference: Block): Project {
  if (!needsG36TimedOverrideMigration(saved)) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  graph.blocks ??= []
  graph.outputs ??= []
  graph.connections ??= []
  for (let zone = 1; zone <= 10; zone++) {
    const name = `zone_${String(zone).padStart(2, '0')}`
    const edgeName = `${name}_override_edge`
    const holdName = `${name}_override_hold`
    const activeName = `${name}_override_active`
    const group = `zone_group_${Math.ceil(zone / 5)}`
    const local = (zone - 1) % 5 + 1
    for (const blockName of [edgeName, holdName]) {
      const template = reference.blocks?.find(block => block.name === blockName)
      if (!template) throw new Error(`G36 timed override migration requires ${blockName}`)
      if (!graph.blocks.some(block => block.name === blockName)) graph.blocks.push(structuredClone(template))
    }
    const output = reference.outputs?.find(port => port.name === activeName)
    if (!output) throw new Error(`G36 timed override migration requires ${activeName}`)
    if (!graph.outputs.some(port => port.name === activeName)) graph.outputs.push(structuredClone(output))
    const targets = new Set([`${group}.override_${local}`, `${name}.local_override`, activeName])
    graph.connections = graph.connections.filter(edge => !targets.has(edgeEndpoints(edge)[1]))
    const present = new Set(graph.connections.map(identity))
    for (const edge of reference.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from !== `${name}_override` && from.split('.')[0] !== edgeName && from.split('.')[0] !== holdName) continue
      if (!present.has(identity(edge))) {
        graph.connections.push(structuredClone(edge))
        present.add(identity(edge))
      }
    }
  }
  graph.notes = `${graph.notes ?? ''} Local override button presses now pass through editable two-hour CDL holds.`.trim()
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
