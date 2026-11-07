import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add CDL source-stage detection and timed water-reset holds to saved G36 plants. */
export function migrateG36WaterStageHold(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const previous = saved.graph.blocks?.find(block => block.name === 'plant')
  if (['hot', 'chilled'].every(loop => previous?.blocks?.some(block => block.name === `${loop}_stage_hold_delay`)
    && previous.blocks.some(block => block.name === `${loop}_reset_output_active`))) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 water-stage migration requires plant')
  for (const block of template.blocks ?? []) {
    if (!block.name.startsWith('hot_stage_') && !block.name.startsWith('chilled_stage_')
      && !['hot_reset_stage_hold', 'chilled_reset_stage_hold',
        'hot_reset_held_value', 'chilled_reset_held_value',
        'hot_reset_output_select', 'chilled_reset_output_select',
        'hot_reset_output_active', 'chilled_reset_output_active'].includes(block.name)) continue
    if (!plant.blocks?.some(existing => existing.name === block.name)) {
      if (!plant.blocks) plant.blocks = []
      plant.blocks.push(structuredClone(block))
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [, to] = edgeEndpoints(edge)
    return !['hot_reset_enabled.u1', 'hot_reset_enabled.u3',
      'chilled_reset_enabled.u1', 'chilled_reset_enabled.u3',
      'hot_water_reset_fraction', 'chilled_water_reset_fraction',
      'hot_reset_inverse.u2', 'chilled_reset_temperature_excess.u1',
      'hot_dp_reset_limited.u1', 'chilled_dp_reset_limited.u1'].includes(to)
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (![from, to].some(endpoint => endpoint.startsWith('hot_stage_')
      || endpoint.startsWith('chilled_stage_')
      || endpoint.startsWith('hot_reset_stage_hold')
      || endpoint.startsWith('chilled_reset_stage_hold')
      || endpoint.startsWith('hot_reset_held_value')
      || endpoint.startsWith('chilled_reset_held_value')
      || endpoint.startsWith('hot_reset_output_')
      || endpoint.startsWith('chilled_reset_output_'))
      && !['hot_reset_enabled.u3', 'chilled_reset_enabled.u3'].includes(to)) continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
