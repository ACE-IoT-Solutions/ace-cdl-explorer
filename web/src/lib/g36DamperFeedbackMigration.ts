import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const feedbackBlocks = new Set([
  'damper_flow_pi', 'damper_flow_threshold', 'damper_flow_requested',
  'damper_closed', 'damper_feedback_output', 'measured_airflow_passthrough',
])

/** Add measured-flow damper control while keeping zone setpoints and tuning. */
export function migrateG36DamperFeedback(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.filter(block => block.block_type === 'VAVBoxController')
    .every(zone => zone.blocks?.some(block => block.name === 'damper_flow_pi'))) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 damper migration requires ${name}`)
    for (const child of template.blocks ?? []) {
      if (feedbackBlocks.has(child.name) && !zone.blocks?.some(block => block.name === child.name)) {
        (zone.blocks ??= []).push(structuredClone(child))
      }
    }
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !(from === 'heating_damper_select.y' && ['damper_position', 'damper_nearly_open.u1'].includes(to))
        && !(from === 'airflow_sum.y' && to === 'airflow')
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!feedbackBlocks.has(from.split('.')[0]) && !feedbackBlocks.has(to.split('.')[0])) continue
      if (['damper_position', 'damper_nearly_open.u1', 'airflow'].includes(to)
        && zone.connections.some(existing => edgeEndpoints(existing)[1] === to)) continue
      if (!zone.connections.some(existing => { const [a, b] = edgeEndpoints(existing); return a === from && b === to })) {
        zone.connections.push({ from, to })
      }
    }
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
