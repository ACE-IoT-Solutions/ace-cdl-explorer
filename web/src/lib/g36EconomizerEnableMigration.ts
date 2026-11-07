import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const newBlocks = new Set([
  'economizer_high_limit_error', 'economizer_high_limit_hysteresis', 'economizer_high_limit_ok',
  'economizer_freeze_early', 'economizer_freeze_any', 'economizer_freeze_clear',
  'economizer_proven_and_clear',
])

/** Qualify saved G36 economizers with raw fan proof, freeze state and high-limit hysteresis. */
export function migrateG36EconomizerEnable(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if ([1, 2].every(index => saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
    ?.blocks?.some(block => block.name === 'economizer_high_limit_hysteresis'))
    && saved.graph.inputs?.some(port => port.name === 'ahu_1_supply_fan_proven')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const addPort = (target: Block, template: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = template[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`G36 economizer reference lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 economizer migration requires ${name}`)
    addPort(graph, reference, 'inputs', `${name}_supply_fan_proven`)
    addPort(ahu, template, 'inputs', 'supply_fan_proven')
    addPort(ahu, template, 'outputs', 'economizer_mode_enabled')
    addPort(graph, reference, 'outputs', `${name}_economizer_mode_enabled`)
    for (const child of template.blocks ?? []) {
      if (newBlocks.has(child.name) && !ahu.blocks?.some(block => block.name === child.name)) {
        (ahu.blocks ??= []).push(structuredClone(child))
      }
    }
    ahu.connections = (ahu.connections ?? []).filter(edge => ![
      'outdoor_below_return.y→free_cooling_useful.u1',
      'fan_requested.y→economizer_temperature_ok.u2',
    ].includes(edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (newBlocks.has(from.split('.')[0]) || newBlocks.has(to.split('.')[0])
        || from === 'supply_fan_proven' || to === 'economizer_mode_enabled') addEdge(ahu, from, to)
    }
    addEdge(graph, `${name}_supply_fan_proven`, `${name}.supply_fan_proven`)
    addEdge(graph, `${name}.economizer_mode_enabled`, `${name}_economizer_mode_enabled`)
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
