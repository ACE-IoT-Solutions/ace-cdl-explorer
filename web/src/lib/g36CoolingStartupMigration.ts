import { edgeEndpoints, layoutFor, type Project } from './types'

/** Keep VAV cooling airflow requested while an AHU starts with warm measured supply air. */
export function migrateG36CoolingStartup(saved: Project): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.filter(block => block.block_type === 'VAVBoxController')
    .every(zone => !zone.blocks?.some(block => block.name === 'cooling_enabled'))) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController') continue
    zone.blocks = zone.blocks?.filter(block => !['air_can_cool', 'cooling_enabled'].includes(block.name))
    zone.connections = zone.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !['air_can_cool', 'cooling_enabled'].includes(from.split('.')[0])
        && !['air_can_cool', 'cooling_enabled'].includes(to.split('.')[0])
    })
    if (!zone.connections?.some(edge => edgeEndpoints(edge).join('→') === 'cooling_mode.y→damper_select.u3')) {
      (zone.connections ??= []).push({ from: 'cooling_mode.y', to: 'damper_select.u3' })
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
