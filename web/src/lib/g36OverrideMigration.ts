import { edgeEndpoints, layoutFor, type Block, type Project } from './types'
import { migrateG36TimedOverrides } from './g36TimedOverrideMigration'

const id = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')
const addedGroupBlock = (name: string) => /^(override_any_|group_occupied$|not_group_occupied$|lookahead_unoccupied$|window_any_|window_alarm_)/.test(name)
const addedZoneBlock = (name: string) => name === 'local_occupied'

/** Add local occupancy overrides and group window alarms without replacing tuning. */
export function migrateG36Overrides(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls' ||
      !saved.graph.blocks?.some(block => block.name === 'zone_group_1') ||
      saved.graph.inputs?.some(port => port.name === 'zone_01_override')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  graph.inputs ??= []
  graph.outputs ??= []
  graph.connections ??= []
  for (const port of reference.inputs ?? []) if (/^zone_\d{2}_override$/.test(port.name)) graph.inputs.push(structuredClone(port))
  for (const port of reference.outputs ?? []) if (/^ahu_[12]_group_(occupied|window_open_alarm_level)$/.test(port.name)) graph.outputs.push(structuredClone(port))
  graph.connections = graph.connections.filter(edge =>
    !/^ahu_[12]\.occupied_mode$/.test(edgeEndpoints(edge)[1]))
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!from.endsWith('_override') && !/^zone_group_[12]\.(occupied|window_open_alarm_level)$/.test(from)) continue
    if (!graph.connections.some(old => id(old) === id(edge))) graph.connections.push(structuredClone(edge))
  }
  for (const group of graph.blocks ?? []) {
    const groupMode = /^zone_group_[12]$/.test(group.name)
    if (!groupMode && !/^zone_\d{2}$/.test(group.name)) continue
    const template = reference.blocks?.find(block => block.name === group.name)
    if (!template) continue
    group.inputs ??= []
    group.outputs ??= []
    group.blocks ??= []
    group.connections ??= []
    for (const port of template.inputs ?? []) if (groupMode ? /^override_[1-5]$/.test(port.name) : port.name === 'local_override')
      if (!group.inputs.some(old => old.name === port.name)) group.inputs.push(structuredClone(port))
    if (groupMode) for (const port of template.outputs ?? []) if (['occupied', 'window_open_alarm_level'].includes(port.name))
      if (!group.outputs.some(old => old.name === port.name)) group.outputs.push(structuredClone(port))
    const isAdded = groupMode ? addedGroupBlock : addedZoneBlock
    const names = new Set((template.blocks ?? []).filter(block => isAdded(block.name)).map(block => block.name))
    for (const block of template.blocks ?? []) if (names.has(block.name) && !group.blocks.some(old => old.name === block.name))
      group.blocks.push(structuredClone(block))
    const rewired = new Set((template.connections ?? []).filter(edge =>
      names.has(edgeEndpoints(edge)[0].split('.')[0]) && !names.has(edgeEndpoints(edge)[1].split('.')[0]))
      .map(edge => edgeEndpoints(edge)[1]))
    group.connections = group.connections.filter(edge => !rewired.has(edgeEndpoints(edge)[1]))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!names.has(from.split('.')[0]) && !names.has(to.split('.')[0]) &&
          !(groupMode ? /^override_[1-5]$/.test(from) || ['occupied', 'window_open_alarm_level'].includes(to) : from === 'local_override')) continue
      if (!group.connections.some(old => id(old) === id(edge))) group.connections.push(structuredClone(edge))
    }
    group.notes = `${group.notes ?? ''} ${groupMode ?
      'Local overrides occupy the group; open windows outside occupied mode raise Level 4.' :
      'Local override selects occupied terminal control through CDL.'}`.trim()
  }
  graph.notes = `${graph.notes ?? ''} Local override contacts feed CDL zone/group occupancy and Level 4 window alarms.`.trim()
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return migrateG36TimedOverrides(next, reference)
}
