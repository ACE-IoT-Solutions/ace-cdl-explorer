import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const isG36 = (graph: Block) => graph.name === 'FourPipeG36Controls' ||
  graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')

export function needsG36AlarmAcknowledgementMigration(saved: Project): boolean {
  if (!isG36(saved.graph)) return false
  const edges = new Set(saved.graph.connections?.map(edge => edgeEndpoints(edge).join('→')) ?? [])
  return [1, 2].some(ahu => {
    const name = `ahu_${ahu}_alarm_acknowledgement`
    if (!saved.graph.blocks?.some(block => block.name === name)) return true
    return [1, 2, 3, 4, 5].some(local => {
      const zone = `zone_${String((ahu - 1) * 5 + local).padStart(2, '0')}`
      const raw = `${zone}_low_airflow_acknowledge`
      const status = `${zone}_low_airflow_acknowledged`
      return !saved.graph.inputs?.some(port => port.name === raw) ||
        !saved.graph.outputs?.some(port => port.name === status) ||
        !edges.has(`${zone}.low_airflow_alarm_level→${name}.alarm_level_${local}`) ||
        !edges.has(`${raw}→${name}.acknowledge_${local}`) ||
        !edges.has(`${name}.acknowledged_${local}→${status}`)
    })
  })
}

/** Add editable acknowledgment state without touching existing alarm calculation blocks. */
export function migrateG36AlarmAcknowledgement(saved: Project, reference: Block): Project {
  if (!needsG36AlarmAcknowledgementMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.blocks ??= []
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const edges = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
  for (const ahu of [1, 2]) {
    const name = `ahu_${ahu}_alarm_acknowledgement`
    if (!next.graph.blocks.some(block => block.name === name)) {
      const template = reference.blocks?.find(block => block.name === name)
      if (!template) throw new Error(`G36 acknowledgment migration requires ${name}`)
      next.graph.blocks.push(structuredClone(template))
    }
    for (let local = 1; local <= 5; local++) {
      const zone = `zone_${String((ahu - 1) * 5 + local).padStart(2, '0')}`
      const raw = `${zone}_low_airflow_acknowledge`
      const status = `${zone}_low_airflow_acknowledged`
      const input = reference.inputs?.find(port => port.name === raw)
      const output = reference.outputs?.find(port => port.name === status)
      if (!input || !output) throw new Error(`G36 acknowledgment migration requires ${zone} ports`)
      if (!next.graph.inputs.some(port => port.name === raw)) next.graph.inputs.push(structuredClone(input))
      if (!next.graph.outputs.some(port => port.name === status)) next.graph.outputs.push(structuredClone(output))
      for (const target of [`${name}.alarm_level_${local}`, `${name}.acknowledge_${local}`, status]) {
        const edge = reference.connections?.find(item => edgeEndpoints(item)[1] === target)
        if (!edge) throw new Error(`G36 acknowledgment migration requires ${target} wire`)
        const key = edgeEndpoints(edge).join('→')
        if (!edges.has(key)) { next.graph.connections.push(structuredClone(edge)); edges.add(key) }
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
