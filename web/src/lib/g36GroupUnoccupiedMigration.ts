import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const newRootBlock = (name: string) => /^zone_\d{2}_unoccupied_(heat|cool)_previous$/.test(name)
const newRootOutput = (name: string) => /^ahu_[12]_group_(setback|setup|freeze_setback|freeze_alarm_level)$/.test(name)
const newGroupInput = (name: string) => name === 'scheduled_occupied' || /^unoccupied_(heat|cool)_[1-5]$/.test(name)
const newGroupOutput = (name: string) => ['setback', 'setup', 'freeze_setback', 'freeze_alarm_level'].includes(name)
const newZoneInput = (name: string) => ['group_setback', 'group_setup', 'group_freeze_setback'].includes(name)
const newZoneOutput = (name: string) => ['unoccupied_heat_target', 'unoccupied_cool_target'].includes(name)
const replacedZoneTargets = new Set([
  'heat_setpoint_active.u2', 'cool_setpoint_active.u2',
  'mode_select_setback.u2', 'mode_select_setup.u2', 'mode_select_cooldown.u3',
])
const hasEdge = (edges: NonNullable<Block['connections']>, from: string, to: string) =>
  edges.some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)

/** Add connected CDL group setback, setup, and freeze modes while retaining edits. */
export function migrateG36GroupUnoccupied(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const existingGroup = saved.graph.blocks?.find(block => block.name === 'zone_group_1')
  if (!existingGroup || saved.graph.blocks?.some(block => block.name === 'zone_01_unoccupied_heat_previous')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  graph.blocks ??= []
  graph.connections ??= []
  graph.outputs ??= []
  graph.blocks.push(...(reference.blocks ?? []).filter(block => newRootBlock(block.name) && !graph.blocks!.some(existing => existing.name === block.name)).map(block => structuredClone(block)))
  graph.outputs.push(...(reference.outputs ?? []).filter(port => newRootOutput(port.name) && !graph.outputs!.some(existing => existing.name === port.name)).map(port => structuredClone(port)))
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    const sourceBlock = from.split('.')[0]
    const targetBlock = to.split('.')[0]
    const newGroupPort = /^zone_group_[12]\./.test(to) && newGroupInput(to.split('.')[1]) ||
      /^zone_group_[12]\./.test(from) && newGroupOutput(from.split('.')[1])
    const newZonePort = /^zone_\d{2}\./.test(to) && newZoneInput(to.split('.')[1]) ||
      /^zone_\d{2}\./.test(from) && newZoneOutput(from.split('.')[1])
    if (!newRootBlock(sourceBlock) && !newRootBlock(targetBlock) && !newRootOutput(to) && !newGroupPort && !newZonePort) continue
    if (!hasEdge(graph.connections, from, to)) graph.connections.push(structuredClone(edge))
  }
  for (let groupIndex = 1; groupIndex <= 2; groupIndex++) {
    const name = `zone_group_${groupIndex}`
    const group = graph.blocks.find(block => block.name === name)!
    const template = reference.blocks?.find(block => block.name === name)!
    group.inputs = [...(group.inputs ?? []), ...(template.inputs ?? []).filter(port => newGroupInput(port.name) && !group.inputs?.some(existing => existing.name === port.name)).map(port => structuredClone(port))]
    group.outputs = [...(group.outputs ?? []), ...(template.outputs ?? []).filter(port => newGroupOutput(port.name) && !group.outputs?.some(existing => existing.name === port.name)).map(port => structuredClone(port))]
    const originalNames = new Set((group.blocks ?? []).map(block => block.name))
    group.blocks = [...(group.blocks ?? []), ...(template.blocks ?? []).filter(block => !originalNames.has(block.name)).map(block => structuredClone(block))]
    group.connections ??= []
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!newGroupInput(from) && !newGroupOutput(to) && originalNames.has(from.split('.')[0]) && originalNames.has(to.split('.')[0])) continue
      if (!hasEdge(group.connections, from, to)) group.connections.push(structuredClone(edge))
    }
    group.description = template.description
    group.notes = `${group.notes ?? ''} Editable latches coordinate group setback, setup, and zone freeze setback.`.trim()
  }
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = graph.blocks.find(block => block.name === name)!
    const template = reference.blocks?.find(block => block.name === name)!
    zone.inputs = [...(zone.inputs ?? []), ...(template.inputs ?? []).filter(port => newZoneInput(port.name) && !zone.inputs?.some(existing => existing.name === port.name)).map(port => structuredClone(port))]
    zone.outputs = [...(zone.outputs ?? []), ...(template.outputs ?? []).filter(port => newZoneOutput(port.name) && !zone.outputs?.some(existing => existing.name === port.name)).map(port => structuredClone(port))]
    const modePort = zone.outputs.find(port => port.name === 'conditioning_state')
    if (modePort) modePort.description = template.outputs?.find(port => port.name === 'conditioning_state')?.description
    const originalNames = new Set((zone.blocks ?? []).map(block => block.name))
    zone.blocks = [...(zone.blocks ?? []), ...(template.blocks ?? []).filter(block => !originalNames.has(block.name)).map(block => structuredClone(block))]
    zone.connections = (zone.connections ?? []).filter(edge => !replacedZoneTargets.has(edgeEndpoints(edge)[1]))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!newZoneInput(from) && !newZoneOutput(to) && !replacedZoneTargets.has(to) &&
          originalNames.has(from.split('.')[0]) && originalNames.has(to.split('.')[0])) continue
      if (!hasEdge(zone.connections, from, to)) zone.connections.push(structuredClone(edge))
    }
    zone.notes = `${zone.notes ?? ''} Unoccupied heating and cooling follow shared group CDL modes.`.trim()
  }
  graph.notes = `${graph.notes ?? ''} Group CDL now commands latched setback, setup, and zone freeze setback.`.trim()
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
