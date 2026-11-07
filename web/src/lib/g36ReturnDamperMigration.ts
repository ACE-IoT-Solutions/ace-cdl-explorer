import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const addedBlocks = new Set([
  'return_damper_one', 'return_damper_complement', 'return_damper_pi_midpoint',
  'return_damper_pi_excess', 'return_damper_zero', 'return_damper_positive_excess',
  'return_damper_double_excess', 'return_damper_pi_max', 'return_damper_oa_limited',
  'return_damper_mode', 'return_damper_physical_max', 'return_damper_physical_limit',
])

/** Add independent return-damper control without replacing saved AHU tuning. */
export function migrateG36ReturnDamper(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if ([1, 2].every(index => saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
    ?.blocks?.some(block => block.name === 'return_damper_physical_limit'))
    && saved.graph.outputs?.some(port => port.name === 'ahu_1_return_air_damper_position')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const addPort = (target: Block, template: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = template[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`G36 return-damper reference lacks ${name}`)
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
    if (!ahu || !template) throw new Error(`G36 return-damper migration requires ${name}`)
    for (const output of ['outdoor_air_damper_position', 'return_air_damper_position']) {
      addPort(ahu, template, 'outputs', output)
      addPort(graph, reference, 'outputs', `${name}_${output}`)
      addEdge(graph, `${name}.${output}`, `${name}_${output}`)
    }
    for (const child of template.blocks ?? []) {
      if (addedBlocks.has(child.name) && !ahu.blocks?.some(block => block.name === child.name)) {
        (ahu.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to === 'return_air_damper_position') continue
      if (addedBlocks.has(from.split('.')[0]) || addedBlocks.has(to.split('.')[0])
        || to === 'outdoor_air_damper_position' || to === 'return_air_damper_position') addEdge(ahu, from, to)
    }
    addEdge(ahu, 'return_damper_physical_limit.y', 'return_air_damper_position')
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
