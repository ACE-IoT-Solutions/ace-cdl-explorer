import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const alarmKinds = [
  'low_airflow',
  'airflow_sensor_alarm',
  'low_discharge_temperature_alarm',
  'leaking_damper_alarm',
  'leaking_reheat_valve_alarm',
  'high_discharge_alarm',
] as const

const isG36 = (graph: Block) => graph.name === 'FourPipeG36Controls' ||
  graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')

function alarmNames(zone: string, local: number, kind: typeof alarmKinds[number]) {
  return kind === 'low_airflow'
    ? { input: `alarm_level_${local}`, output: `acknowledged_${local}`,
      source: `${zone}.low_airflow_alarm_level`, status: `${zone}_low_airflow_acknowledged` }
    : { input: `${kind}_level_${local}`, output: `${kind}_acknowledged_${local}`,
      source: `${zone}.${kind}_level`, status: `${zone}_${kind}_acknowledged` }
}

export function needsG36AlarmAcknowledgementMigration(saved: Project): boolean {
  if (!isG36(saved.graph)) return false
  const edges = new Set(saved.graph.connections?.map(edge => edgeEndpoints(edge).join('→')) ?? [])
  return [1, 2].some(ahu => {
    const name = `ahu_${ahu}_alarm_acknowledgement`
    const group = saved.graph.blocks?.find(block => block.name === name)
    if (!group) return true
    const innerEdges = new Set(group.connections?.map(edge => edgeEndpoints(edge).join('→')) ?? [])
    return [1, 2, 3, 4, 5].some(local => {
      const zone = `zone_${String((ahu - 1) * 5 + local).padStart(2, '0')}`
      const raw = `${zone}_alarm_acknowledge`
      if (!saved.graph.inputs?.some(port => port.name === raw) ||
          !group.inputs?.some(port => port.name === `acknowledge_${local}`) ||
          !edges.has(`${raw}→${name}.acknowledge_${local}`)) return true
      return alarmKinds.some(kind => {
        const {input, output, source, status} = alarmNames(zone, local, kind)
        const latch = kind === 'low_airflow' ? `${zone}_ack_latch` : `${zone}_${kind}_ack_latch`
        return !group.inputs?.some(port => port.name === input) ||
          !group.outputs?.some(port => port.name === output) ||
          !group.blocks?.some(block => block.name === latch) ||
          !innerEdges.has(`${latch}.y→${output}`) ||
          !saved.graph.outputs?.some(port => port.name === status) ||
          !edges.has(`${source}→${name}.${input}`) ||
          !edges.has(`${name}.${output}→${status}`)
      })
    })
  })
}

/** Add editable per-alarm acknowledgment state while retaining saved block edits. */
export function migrateG36AlarmAcknowledgement(saved: Project, reference: Block): Project {
  if (!needsG36AlarmAcknowledgementMigration(saved)) return saved
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
    const name = `ahu_${ahu}_alarm_acknowledgement`
    const template = reference.blocks?.find(block => block.name === name)
    if (!template) throw new Error(`G36 acknowledgment migration requires ${name}`)
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
    for (let local = 1; local <= 5; local++) {
      const zone = `zone_${String((ahu - 1) * 5 + local).padStart(2, '0')}`
      const raw = `${zone}_alarm_acknowledge`
      const oldRaw = `${zone}_low_airflow_acknowledge`
      next.graph.connections = next.graph.connections.filter(edge => edgeEndpoints(edge).join('→') !==
        `${oldRaw}→${name}.acknowledge_${local}`)
      if (!next.graph.connections.some(edge => edgeEndpoints(edge)[0] === oldRaw))
        next.graph.inputs = next.graph.inputs.filter(port => port.name !== oldRaw)
      const input = reference.inputs?.find(port => port.name === raw)
      if (!input) throw new Error(`G36 acknowledgment migration requires ${raw}`)
      mergeNamed(next.graph.inputs, [input])
      for (const kind of alarmKinds) {
        const {input: levelInput, output, source, status} = alarmNames(zone, local, kind)
        const statusPort = reference.outputs?.find(port => port.name === status)
        if (!statusPort) throw new Error(`G36 acknowledgment migration requires ${status}`)
        mergeNamed(next.graph.outputs, [statusPort])
        const wires = reference.connections?.filter(edge => {
          const [from, to] = edgeEndpoints(edge)
          return (from === source && to === `${name}.${levelInput}`) ||
            (from === `${name}.${output}` && to === status)
        }) ?? []
        if (wires.length !== 2) throw new Error(`G36 acknowledgment migration requires ${status} wires`)
        mergeEdges(next.graph.connections, wires)
      }
      const buttonWire = reference.connections?.find(edge => edgeEndpoints(edge).join('→') ===
        `${raw}→${name}.acknowledge_${local}`)
      if (!buttonWire) throw new Error(`G36 acknowledgment migration requires ${raw} wire`)
      mergeEdges(next.graph.connections, [buttonWire])
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
