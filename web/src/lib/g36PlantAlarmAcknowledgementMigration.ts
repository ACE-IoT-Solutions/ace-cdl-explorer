import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const faults = [
  'heat_pump_start_fault', 'boiler_start_fault', 'chiller_start_fault',
  'hot_water_pump_start_fault', 'chilled_water_pump_start_fault',
  'hot_water_flow_fault', 'chilled_water_flow_fault',
] as const
const groupName = 'plant_alarm_acknowledgement'
const rawName = 'plant_alarm_acknowledge'
const isG36 = (graph: Block) => graph.name === 'FourPipeG36Controls' ||
  graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')

export function needsG36PlantAlarmAcknowledgementMigration(saved: Project): boolean {
  if (!isG36(saved.graph)) return false
  const group = saved.graph.blocks?.find(block => block.name === groupName)
  const edges = new Set(saved.graph.connections?.map(edge => edgeEndpoints(edge).join('→')) ?? [])
  if (!group || !saved.graph.inputs?.some(port => port.name === rawName) ||
      !edges.has(`${rawName}→${groupName}.acknowledge`)) return true
  const inner = new Set(group.connections?.map(edge => edgeEndpoints(edge).join('→')) ?? [])
  return faults.some(fault => {
    const status = `plant_${fault}_acknowledged`
    return !group.inputs?.some(port => port.name === fault) ||
      !group.outputs?.some(port => port.name === `${fault}_acknowledged`) ||
      !group.blocks?.some(block => block.name === `${fault}_ack_latch`) ||
      !inner.has(`${fault}_ack_latch.y→${fault}_acknowledged`) ||
      !saved.graph.outputs?.some(port => port.name === status) ||
      !edges.has(`plant.${fault}→${groupName}.${fault}`) ||
      !edges.has(`${groupName}.${fault}_acknowledged→${status}`)
  })
}

/** Add independent plant-fault acknowledgment without replacing tuned plant controls. */
export function migrateG36PlantAlarmAcknowledgement(saved: Project, reference: Block): Project {
  if (!needsG36PlantAlarmAcknowledgementMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.blocks ??= []
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const template = reference.blocks?.find(block => block.name === groupName)
  if (!template) throw new Error('Plant alarm acknowledgment migration requires its CDL composite')
  let group = next.graph.blocks.find(block => block.name === groupName)
  if (!group) { group = structuredClone(template); next.graph.blocks.push(group) }
  else {
    const mergeNamed = <T extends {name: string}>(target: T[], source: T[]) => {
      for (const item of source) if (!target.some(existing => existing.name === item.name)) target.push(structuredClone(item))
    }
    group.inputs ??= []
    group.outputs ??= []
    group.blocks ??= []
    group.connections ??= []
    mergeNamed(group.inputs, template.inputs ?? [])
    mergeNamed(group.outputs, template.outputs ?? [])
    mergeNamed(group.blocks, template.blocks ?? [])
    const innerKeys = new Set(group.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const key = edgeEndpoints(edge).join('→')
      if (!innerKeys.has(key)) { group.connections.push(structuredClone(edge)); innerKeys.add(key) }
    }
  }
  const input = reference.inputs?.find(port => port.name === rawName)
  if (!input) throw new Error('Plant alarm acknowledgment migration requires its raw input')
  if (!next.graph.inputs.some(port => port.name === rawName)) next.graph.inputs.push(structuredClone(input))
  for (const fault of faults) {
    const status = `plant_${fault}_acknowledged`
    const port = reference.outputs?.find(item => item.name === status)
    if (!port) throw new Error(`Plant alarm acknowledgment migration requires ${status}`)
    if (!next.graph.outputs.some(item => item.name === status)) next.graph.outputs.push(structuredClone(port))
  }
  const targets = new Set([rawName, ...faults.map(fault => `plant_${fault}_acknowledged`)])
  const rootEdges = reference.connections?.filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return (from === rawName && to === `${groupName}.acknowledge`) ||
      (from.startsWith('plant.') && to.startsWith(`${groupName}.`)) ||
      targets.has(to)
  }) ?? []
  if (rootEdges.length !== 1 + faults.length * 2)
    throw new Error('Plant alarm acknowledgment migration requires fifteen boundary wires')
  const keys = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
  for (const edge of rootEdges) {
    const key = edgeEndpoints(edge).join('→')
    if (!keys.has(key)) { next.graph.connections.push(structuredClone(edge)); keys.add(key) }
  }
  next.layout = {...layoutFor(next.graph), ...saved.layout}
  return next
}
