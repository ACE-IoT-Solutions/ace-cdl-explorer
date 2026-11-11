import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export const reallocationBlockNames = [
  'heat_pump_shared_after_heat', 'heat_pump_shared_after_cool', 'heat_pump_shared_remaining',
  'heat_pump_heating_service_limit', 'heat_pump_heating_headroom', 'heat_pump_heating_extra',
  'heat_pump_heating_final', 'heat_pump_shared_after_heating_extra',
  'heat_pump_cooling_service_limit', 'heat_pump_cooling_headroom', 'heat_pump_cooling_extra',
  'heat_pump_cooling_final',
]
const newNames = new Set(reallocationBlockNames)
export function needsG36CapacityReallocation(saved: Project): boolean {
  const plant = saved.graph.blocks?.find(block => block.name === 'plant')
  if (!saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController') ||
      !plant?.blocks?.some(block => block.name === 'heat_pump_heating_service_cap') ||
      !plant.blocks?.some(block => block.name === 'heat_pump_cooling_service_cap')) return false
  return reallocationBlockNames.some(name => !plant.blocks?.some(block => block.name === name)) ||
    (['heating', 'cooling'] as const).some(service => {
      const target = `${service === 'heating' ? 'boiler' : 'chiller'}_residual.u2`
      const driver = plant.connections?.find(edge => edgeEndpoints(edge)[1] === target)
      return !driver || edgeEndpoints(driver)[0] === `heat_pump_${service}_service_cap.y`
    })
}

/** Preserve edited dispatch blocks while adding the stock capacity-reuse path. */
export function migrateG36CapacityReallocation(saved: Project, reference: Block): Project {
  if (!needsG36CapacityReallocation(saved)) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks!.find(block => block.name === 'plant')!
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!template) throw new Error('G36 capacity reallocation requires a plant reference')
  plant.blocks ??= []
  plant.connections ??= []
  for (const name of reallocationBlockNames) {
    if (!plant.blocks.some(block => block.name === name)) {
      const source = template.blocks?.find(block => block.name === name)
      if (!source) throw new Error(`G36 capacity reallocation reference is missing ${name}`)
      plant.blocks.push(structuredClone(source))
    }
  }
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    const targetBlock = to.split('.')[0]
    const sourceBlock = from.split('.')[0]
    if (newNames.has(targetBlock)) {
      // An edited driver on an existing input takes precedence over the
      // template. Only complete a newly added or disconnected input.
      if (!plant.connections.some(existing => edgeEndpoints(existing)[1] === to)) {
        plant.connections.push(structuredClone(edge))
      }
    } else if (newNames.has(sourceBlock)) {
      const driver = plant.connections.find(existing => edgeEndpoints(existing)[1] === to)
      if (!driver) {
        plant.connections.push(structuredClone(edge))
      } else if (edgeEndpoints(driver)[0] === `heat_pump_${sourceBlock.includes('heating') ? 'heating' : 'cooling'}_service_cap.y`) {
        driver.from = from
      }
    }
  }
  next.layout = {...layoutFor(next.graph), ...saved.layout}
  return next
}
