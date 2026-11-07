import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const endpoints = (edge: NonNullable<Block['connections']>[number]) => edgeEndpoints(edge).join('→')
const windowBlock = (name: string) => name.startsWith('window_') || name.includes('_window_')

/** Add window contacts and CDL gates to saved G36 projects without replacing tuning. */
export function migrateG36Windows(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls' ||
      !saved.graph.blocks?.some(block => block.name === 'zone_group_1') ||
      saved.graph.inputs?.some(port => port.name === 'zone_01_window_closed')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  graph.inputs ??= []
  graph.connections ??= []
  for (const port of reference.inputs ?? []) if (/^zone_\d{2}_window_closed$/.test(port.name)) graph.inputs.push(structuredClone(port))
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (from.endsWith('_window_closed') && !graph.connections.some(old => endpoints(old) === endpoints(edge)))
      graph.connections.push(structuredClone(edge))
  }
  for (const group of graph.blocks ?? []) {
    if (!/^zone_group_[12]$/.test(group.name) && !/^zone_\d{2}$/.test(group.name)) continue
    const template = reference.blocks?.find(block => block.name === group.name)
    if (!template) continue
    group.inputs ??= []
    group.blocks ??= []
    group.connections ??= []
    for (const port of template.inputs ?? [])
      if (port.name === 'window_closed' || /^window_closed_[1-5]$/.test(port.name))
        if (!group.inputs.some(old => old.name === port.name)) group.inputs.push(structuredClone(port))
    const additions = (template.blocks ?? []).filter(block => windowBlock(block.name))
    const names = new Set(additions.map(block => block.name))
    for (const block of additions) if (!group.blocks.some(old => old.name === block.name)) group.blocks.push(structuredClone(block))
    const rewiredTargets = new Set((template.connections ?? []).filter(edge =>
      names.has(edgeEndpoints(edge)[0].split('.')[0]) && !names.has(edgeEndpoints(edge)[1].split('.')[0]))
      .map(edge => edgeEndpoints(edge)[1]))
    group.connections = group.connections.filter(edge => !rewiredTargets.has(edgeEndpoints(edge)[1]))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!names.has(from.split('.')[0]) && !names.has(to.split('.')[0]) &&
          from !== 'window_closed' && !/^window_closed_[1-5]$/.test(from)) continue
      if (!group.connections.some(old => endpoints(old) === endpoints(edge))) group.connections.push(structuredClone(edge))
    }
    group.notes = `${group.notes ?? ''} ${/^zone_group/.test(group.name) ?
      'Open-window zones are excluded from ordinary group decisions; freeze protection still sees every zone.' :
      'Open windows disable ordinary conditioning while group freeze setback retains heating.'}`.trim()
  }
  graph.notes = `${graph.notes ?? ''} Raw window contacts feed editable CDL group exclusion and terminal gates.`.trim()
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
