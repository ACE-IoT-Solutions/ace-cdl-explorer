import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const alarms = [
  'freeze_ventilation_alarm',
  'freeze_alarm',
  'supply_fan_alarm',
  'fan_speed_tracking_alarm',
  'outdoor_damper_tracking_alarm',
  'return_damper_tracking_alarm',
] as const

const isG36 = (graph: Block) => graph.name === 'FourPipeG36Controls' ||
  graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')

export function needsG36AhuAlarmAcknowledgementMigration(saved: Project): boolean {
  if (!isG36(saved.graph)) return false
  const edges = new Set(saved.graph.connections?.map(edge => edgeEndpoints(edge).join('→')) ?? [])
  return [1, 2].some(ahu => {
    const prefix = `ahu_${ahu}`
    const name = `${prefix}_equipment_alarm_acknowledgement`
    const group = saved.graph.blocks?.find(block => block.name === name)
    const raw = `${prefix}_alarm_acknowledge`
    if (!group || !saved.graph.inputs?.some(port => port.name === raw) ||
        !edges.has(`${raw}→${name}.acknowledge`)) return true
    const innerEdges = new Set(group.connections?.map(edge => edgeEndpoints(edge).join('→')) ?? [])
    return alarms.some(alarm => {
      const source = `${prefix}_${alarm}_level`
      const status = `${prefix}_${alarm}_acknowledged`
      return !group.inputs?.some(port => port.name === source) ||
        !group.outputs?.some(port => port.name === status) ||
        !group.blocks?.some(block => block.name === `${alarm}_ack_latch`) ||
        !innerEdges.has(`${alarm}_ack_latch.y→${status}`) ||
        !saved.graph.outputs?.some(port => port.name === status) ||
        !edges.has(`${prefix}.${alarm}_level→${name}.${source}`) ||
        !edges.has(`${name}.${status}→${status}`)
    })
  })
}

/** Restore missing AHU alarm channels from the bundled CDL reference, preserving edits. */
export function migrateG36AhuAlarmAcknowledgement(saved: Project, reference: Block): Project {
  if (!needsG36AhuAlarmAcknowledgementMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.blocks ??= []
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const mergeNamed = <T extends {name: string}>(target: T[], source: T[]) => {
    for (const item of source) if (!target.some(existing => existing.name === item.name)) target.push(structuredClone(item))
  }
  const mergeEdges = (target: NonNullable<Block['connections']>, source: NonNullable<Block['connections']>) => {
    const keys = new Set(target.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of source) {
      const key = edgeEndpoints(edge).join('→')
      if (!keys.has(key)) { target.push(structuredClone(edge)); keys.add(key) }
    }
  }
  for (const ahu of [1, 2]) {
    const prefix = `ahu_${ahu}`
    const name = `${prefix}_equipment_alarm_acknowledgement`
    const template = reference.blocks?.find(block => block.name === name)
    if (!template) throw new Error(`AHU alarm acknowledgment migration requires ${name}`)
    let group = next.graph.blocks.find(block => block.name === name)
    if (!group) { group = structuredClone(template); next.graph.blocks.push(group) }
    else {
      group.inputs ??= []
      group.outputs ??= []
      group.blocks ??= []
      group.connections ??= []
      mergeNamed(group.inputs, template.inputs ?? [])
      mergeNamed(group.outputs, template.outputs ?? [])
      mergeNamed(group.blocks, template.blocks ?? [])
      mergeEdges(group.connections, template.connections ?? [])
    }
    const raw = `${prefix}_alarm_acknowledge`
    const rawPort = reference.inputs?.find(port => port.name === raw)
    if (!rawPort) throw new Error(`AHU alarm acknowledgment migration requires ${raw}`)
    mergeNamed(next.graph.inputs, [rawPort])
    for (const alarm of alarms) {
      const source = `${prefix}_${alarm}_level`
      const status = `${prefix}_${alarm}_acknowledged`
      const port = reference.outputs?.find(item => item.name === status)
      if (!port) throw new Error(`AHU alarm acknowledgment migration requires ${status}`)
      mergeNamed(next.graph.outputs, [port])
      const wires = reference.connections?.filter(edge => {
        const [from, to] = edgeEndpoints(edge)
        return (from === `${prefix}.${alarm}_level` && to === `${name}.${source}`) ||
          (from === `${name}.${status}` && to === status)
      }) ?? []
      if (wires.length !== 2) throw new Error(`AHU alarm acknowledgment migration requires ${status} wires`)
      mergeEdges(next.graph.connections, wires)
    }
    const buttonWire = reference.connections?.find(edge => edgeEndpoints(edge).join('→') ===
      `${raw}→${name}.acknowledge`)
    if (!buttonWire) throw new Error(`AHU alarm acknowledgment migration requires ${raw} wire`)
    mergeEdges(next.graph.connections, [buttonWire])
  }
  next.layout = {...layoutFor(next.graph), ...saved.layout}
  return next
}
