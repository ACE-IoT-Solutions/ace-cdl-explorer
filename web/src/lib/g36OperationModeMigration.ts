import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const added = (name: string): boolean => name === 'obc_operation_mode' || name.startsWith('mode_')
const controlTargets = new Set([
  'warmup', 'cooldown', 'setback', 'setup', 'freeze_setback',
  'any_preconditioning.u1', 'any_preconditioning.u2',
  'optimal_warmup_active.u', 'optimal_cooldown_active.u',
  'freeze_alarm_select.u2',
])

export function needsG36OperationModeMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  return [1, 2].some(index => saved.graph.blocks?.find(block => block.name === `zone_group_${index}`)
    ?.blocks?.every(block => block.name !== 'obc_operation_mode'))
}

/** Replace teaching-mode outputs with the pinned OBC decision plus CDL arbitration. */
export function migrateG36OperationMode(saved: Project, reference: Block): Project {
  if (!needsG36OperationModeMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `zone_group_${index}`
    const group = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!group || !template) throw new Error(`G36 operation-mode migration requires ${name}`)
    group.blocks ??= []
    group.connections ??= []
    if (group.blocks.some(block => block.name === 'obc_operation_mode')) continue
    for (const block of template.blocks ?? []) {
      if (added(block.name) && !group.blocks.some(current => current.name === block.name))
        group.blocks.push(structuredClone(block))
    }
    const oldValue = (blockName: string): number | undefined => {
      const value = group.blocks?.find(block => block.name === blockName)
        ?.parameters?.find(parameter => parameter.name === 'k')?.value
      return typeof value === 'number' && Number.isFinite(value) ? value : undefined
    }
    const composite = group.blocks.find(block => block.name === 'obc_operation_mode')
    for (const [oldName, childName, parameterName] of [
      ['maximum_start_seconds', 'maxWarCooTime', 'k'],
      ['maximum_start_seconds', 'truFalHol', 'trueHoldDuration'],
      ['maximum_start_seconds', 'truFalHol1', 'trueHoldDuration'],
      ['freeze_setback_on', 'actFreProTem', 'k'],
      ['freeze_setback_off', 'endFreProTem', 'k'],
    ]) {
      const value = oldValue(oldName)
      const parameter = composite?.blocks?.find(block => block.name === childName)
        ?.parameters?.find(parameter => parameter.name === parameterName)
      if (value !== undefined && parameter) parameter.value = value
    }
    const existingTargets = new Map(group.connections.map(edge => {
      const [from, to] = edgeEndpoints(edge)
      return [to, from]
    }))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!added(from.split('.')[0]) && !added(to.split('.')[0])) continue
      if (controlTargets.has(to)) {
        group.connections = group.connections.filter(current => edgeEndpoints(current)[1] !== to)
        group.connections.push(structuredClone(edge))
        existingTargets.set(to, from)
      } else if (!existingTargets.has(to)) {
        group.connections.push(structuredClone(edge))
        existingTargets.set(to, from)
      }
    }
    const modePort = template.outputs?.find(port => port.name === 'operation_mode')
    if (modePort && !group.outputs?.some(port => port.name === 'operation_mode')) {
      group.outputs ??= []
      group.outputs.push(structuredClone(modePort))
    }
    group.notes = `${group.notes ?? ''} OBC OperationMode now supplies requests; CDL priority switches select one mode.`.trim()
  }
  for (const index of [1, 2]) {
    const name = `ahu_${index}_group_operation_mode`
    const port = reference.outputs?.find(port => port.name === name)
    const edge = reference.connections?.find(connection => edgeEndpoints(connection)[1] === name)
    if (port && !next.graph.outputs?.some(current => current.name === name)) {
      next.graph.outputs ??= []
      next.graph.outputs.push(structuredClone(port))
    }
    if (edge && !next.graph.connections?.some(connection => edgeEndpoints(connection)[1] === name)) {
      next.graph.connections ??= []
      next.graph.connections.push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
