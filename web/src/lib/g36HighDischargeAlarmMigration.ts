import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const isG36 = (graph: Block) => graph.name === 'FourPipeG36Controls' ||
  graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')

export function needsG36HighDischargeAlarmMigration(saved: Project): boolean {
  return Boolean(isG36(saved.graph) && saved.graph.blocks?.some(zone =>
    zone.block_type === 'VAVBoxController' &&
    !zone.blocks?.some(block => block.name === 'high_discharge_condition')))
}

/** Gate the existing timed alarm with CDL occupied and fan proof; keep its timer tuning. */
export function migrateG36HighDischargeAlarm(saved: Project, reference: Block): Project {
  if (!needsG36HighDischargeAlarmMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController') continue
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`G36 high-discharge migration requires ${zone.name}`)
    zone.blocks ??= []
    zone.connections ??= []
    for (const name of ['high_discharge_eligible', 'high_discharge_condition']) {
      if (zone.blocks.some(block => block.name === name)) continue
      const block = template.blocks?.find(block => block.name === name)
      if (!block) throw new Error(`G36 high-discharge migration requires ${zone.name}.${name}`)
      zone.blocks.push(structuredClone(block))
    }
    zone.connections = zone.connections.filter(edge => edgeEndpoints(edge).join('→') !==
      'high_discharge_detected.y→high_discharge_delay.u')
    const present = new Set(zone.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [, to] = edgeEndpoints(edge)
      if (!to.startsWith('high_discharge_eligible.') &&
          !to.startsWith('high_discharge_condition.') &&
          to !== 'high_discharge_delay.u') continue
      const key = edgeEndpoints(edge).join('→')
      if (!present.has(key)) { zone.connections.push(structuredClone(edge)); present.add(key) }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
