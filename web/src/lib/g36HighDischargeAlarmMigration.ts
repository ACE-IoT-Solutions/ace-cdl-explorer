import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const isG36 = (graph: Block) => graph.name === 'FourPipeG36Controls' ||
  graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')
const hasEdge = (graph: Block, from: string, to: string) =>
  graph.connections?.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`) ?? false

export function needsG36HighDischargeAlarmMigration(saved: Project): boolean {
  return Boolean(isG36(saved.graph) && saved.graph.blocks?.some(zone =>
    zone.block_type === 'VAVBoxController' && (
      !zone.blocks?.some(block => block.name === 'high_discharge_condition') ||
      !zone.blocks?.some(block => block.name === 'high_discharge_alarm_priority') ||
      !zone.outputs?.some(port => port.name === 'high_discharge_alarm_level') ||
      !saved.graph.outputs?.some(port => port.name === `${zone.name}_high_discharge_alarm_level`) ||
      !hasEdge(zone, 'high_discharge_delay.y', 'high_discharge_alarm_priority.u') ||
      !hasEdge(zone, 'high_discharge_alarm_priority.y', 'high_discharge_alarm_level') ||
      !hasEdge(saved.graph, `${zone.name}.high_discharge_alarm_level`, `${zone.name}_high_discharge_alarm_level`))))
}

/** Gate the timed alarm and expose its editable teaching level without losing timer tuning. */
export function migrateG36HighDischargeAlarm(saved: Project, reference: Block): Project {
  if (!needsG36HighDischargeAlarmMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const addEdges = (target: NonNullable<Block['connections']>, source: NonNullable<Block['connections']>) => {
    const present = new Set(target.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of source) {
      const key = edgeEndpoints(edge).join('→')
      if (!present.has(key)) { target.push(structuredClone(edge)); present.add(key) }
    }
  }
  for (const zone of next.graph.blocks ?? []) {
    if (zone.block_type !== 'VAVBoxController') continue
    const template = reference.blocks?.find(block => block.name === zone.name)
    if (!template) throw new Error(`G36 high-discharge migration requires ${zone.name}`)
    zone.blocks ??= []
    zone.connections ??= []
    zone.outputs ??= []
    const levelPort = template.outputs?.find(port => port.name === 'high_discharge_alarm_level')
    if (!levelPort) throw new Error(`G36 high-discharge migration requires ${zone.name}.high_discharge_alarm_level`)
    if (!zone.outputs.some(port => port.name === levelPort.name)) zone.outputs.push(structuredClone(levelPort))
    for (const name of ['high_discharge_eligible', 'high_discharge_condition', 'high_discharge_alarm_priority']) {
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
          to !== 'high_discharge_delay.u' &&
          to !== 'high_discharge_alarm_priority.u' &&
          to !== 'high_discharge_alarm_level') continue
      const key = edgeEndpoints(edge).join('→')
      if (!present.has(key)) { zone.connections.push(structuredClone(edge)); present.add(key) }
    }
    const rootPortName = `${zone.name}_high_discharge_alarm_level`
    const rootPort = reference.outputs?.find(port => port.name === rootPortName)
    if (!rootPort) throw new Error(`G36 high-discharge migration requires ${rootPortName}`)
    if (!next.graph.outputs.some(port => port.name === rootPortName)) next.graph.outputs.push(structuredClone(rootPort))
    addEdges(next.graph.connections, reference.connections?.filter(edge =>
      edgeEndpoints(edge).join('→') === `${zone.name}.high_discharge_alarm_level→${rootPortName}`) ?? [])
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
