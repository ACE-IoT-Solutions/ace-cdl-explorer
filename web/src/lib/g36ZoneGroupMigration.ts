import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const isNewRootBlock = (name: string) => name === 'any_group_preconditioning' ||
  /^zone_group_[12]$/.test(name) || /^zone_\d{2}_(heat|cool)_target_previous$/.test(name)
const newZoneInputs = new Set(['group_warmup', 'group_cooldown'])
const newZoneOutputs = new Set(['occupied_heat_target', 'occupied_cool_target'])
const rewiredZoneTargets = new Set(['warmup_mode.u1', 'cooldown_mode.u1'])

/** Add two five-zone CDL coordinators and preserve editable zone setpoints. */
export function migrateG36ZoneGroups(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.some(block => block.name === 'zone_group_1')) return saved
  const next = structuredClone(saved)
  const nextGraph = next.graph
  for (const block of reference.blocks ?? []) {
    if (isNewRootBlock(block.name)) nextGraph.blocks = [...(nextGraph.blocks ?? []), structuredClone(block)]
  }
  nextGraph.outputs = [...(nextGraph.outputs ?? []), ...(reference.outputs ?? [])
    .filter(port => /^ahu_[12]_group_(warmup|cooldown|preconditioning)$/.test(port.name))
    .map(port => structuredClone(port))]
  const originalMode = nextGraph.outputs.find(port => port.name === 'preconditioning_mode')
  const templateMode = reference.outputs?.find(port => port.name === 'preconditioning_mode')
  if (originalMode && templateMode) originalMode.description = templateMode.description
  nextGraph.connections = (nextGraph.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return to !== 'preconditioning_mode' &&
      !(from === 'preconditioning_enabled.y' && /^zone_\d{2}\.preconditioning_mode$/.test(to))
  })
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!isNewRootBlock(from.split('.')[0]) && !isNewRootBlock(to.split('.')[0]) &&
      to !== 'preconditioning_mode' &&
      !/^ahu_[12]_group_(warmup|cooldown|preconditioning)$/.test(to)) continue
    if (!nextGraph.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      nextGraph.connections.push(structuredClone(edge))
    }
  }
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = nextGraph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 zone group migration requires ${name}`)
    const setback = zone.blocks?.find(block => block.name === 'unoccupied_heat')?.parameters?.find(parameter => parameter.name === 'k')
    if (setback?.value === 16) setback.value = 18
    zone.inputs = (zone.inputs ?? []).filter(port => port.name !== 'preconditioning_mode')
    zone.inputs.push(...(template.inputs ?? []).filter(port => newZoneInputs.has(port.name)).map(port => structuredClone(port)))
    zone.outputs = [...(zone.outputs ?? []), ...(template.outputs ?? [])
      .filter(port => newZoneOutputs.has(port.name)).map(port => structuredClone(port))]
    zone.connections = (zone.connections ?? []).filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !(from === 'preconditioning_mode' && rewiredZoneTargets.has(to))
    })
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!newZoneInputs.has(from) && !newZoneOutputs.has(to)) continue
      if (!zone.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
        zone.connections.push(structuredClone(edge))
      }
    }
  }
  next.layout = { ...layoutFor(nextGraph), ...saved.layout }
  return next
}
