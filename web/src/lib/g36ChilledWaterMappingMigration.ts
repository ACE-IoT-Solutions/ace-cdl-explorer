import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Move chilled-water temperature reset into the upper half of plant reset. */
export function migrateG36ChilledWaterMapping(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const oldPlant = saved.graph.blocks?.find(block => block.name === 'plant')
  if (oldPlant?.blocks?.some(block => block.name === 'chilled_reset_temperature_scale')) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 chilled-water mapping requires plant')
  for (const block of template.blocks ?? []) {
    if (block.name.startsWith('chilled_reset_temperature_')) (plant.blocks ??= []).push(structuredClone(block))
  }
  plant.connections = (plant.connections ?? []).filter(edge =>
    edgeEndpoints(edge).join('→') !== 'chilled_reset_previous.y→chilled_reset_inverse.u2')
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!from.startsWith('chilled_reset_temperature_') && !to.startsWith('chilled_reset_temperature_')) continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
