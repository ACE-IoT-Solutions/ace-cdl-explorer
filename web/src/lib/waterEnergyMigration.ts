import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const names = new Set([
  'source_zero', 'hot_water_source_error', 'chilled_water_source_error',
  'hot_water_source_gain', 'chilled_water_source_gain',
  'hot_water_source_sum', 'chilled_water_source_sum',
  'hot_water_source_nonnegative', 'chilled_water_source_nonnegative',
])
const dispatchTargets = new Set([
  'heat_pump_heating.u1', 'heat_pump_cooling.u1', 'boiler.u1', 'chiller.u1',
])

/** Make source production depend on CDL temperature feedback and predicted load. */
export function migrateWaterEnergyControl(saved: Project, reference: Block): Project {
  if (!['FourPipeBuildingControls', 'FourPipeG36Controls'].includes(saved.graph.name)) return saved
  if (saved.graph.blocks?.find(block => block.name === 'plant')?.blocks?.some(block => block.name === 'hot_water_source_gain')) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('Water energy migration requires the plant composite')
  for (const block of template.blocks ?? []) {
    if (names.has(block.name) && !plant.blocks?.some(item => item.name === block.name)) {
      (plant.blocks ??= []).push(structuredClone(block))
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !(dispatchTargets.has(to) && ['heat_request_kw', 'cool_request_kw'].includes(from))
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!names.has(from.split('.')[0]) && !names.has(to.split('.')[0])) continue
    if (dispatchTargets.has(to) && plant.connections.some(existing => edgeEndpoints(existing)[1] === to)) continue
    if (!plant.connections.some(existing => { const [a, b] = edgeEndpoints(existing); return a === from && b === to })) {
      plant.connections.push({ from, to })
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
