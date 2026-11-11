import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const comparisons = [
  ['freeze_stage_one_cold', 'Less'],
  ['freeze_stage_one_warm', 'Greater'],
  ['freeze_stage_two_cold', 'Less'],
  ['freeze_stage_three_below_fifteen', 'Less'],
  ['freeze_stage_three_below_five', 'Less'],
] as const

export function needsG36FreezeThresholds(saved: Project): boolean {
  if (!saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return false
  return saved.graph.blocks.some(ahu => ahu.block_type === 'AHUController' && comparisons.some(([name, kind]) =>
    ahu.blocks?.some(block => block.name === name && block.block_type.split('.').at(-1) === kind)))
}

/** Preserve each saved threshold source and replace plain comparisons with OBC hysteresis. */
export function migrateG36FreezeThresholds(saved: Project, reference: Block): Project {
  if (!needsG36FreezeThresholds(saved)) return saved
  const next = structuredClone(saved)
  for (const ahu of next.graph.blocks ?? []) {
    if (ahu.block_type !== 'AHUController') continue
    const template = reference.blocks?.find(block => block.name === ahu.name)
    if (!template) throw new Error(`G36 freeze threshold reference lacks ${ahu.name}`)
    for (const [name, kind] of comparisons) {
      const old = ahu.blocks?.find(block => block.name === name)
      if (!old || old.block_type.split('.').at(-1) !== kind) continue
      const incoming = (ahu.connections ?? []).filter(edge => edgeEndpoints(edge)[1].startsWith(`${name}.`))
      const first = incoming.find(edge => edgeEndpoints(edge)[1] === `${name}.u1`)
      const second = incoming.find(edge => edgeEndpoints(edge)[1] === `${name}.u2`)
      if (!first || !second || incoming.length !== 2) continue
      const errorName = `${name}_error`
      if (ahu.blocks?.some(block => block.name === errorName)) continue
      const error = template.blocks?.find(block => block.name === errorName)
      const threshold = template.blocks?.find(block => block.name === name)
      if (!error || !threshold) throw new Error(`G36 freeze threshold reference lacks ${name}`)
      const notes = old.notes
      Object.assign(old, structuredClone(threshold))
      if (notes && notes !== threshold.notes) old.notes = `${notes} ${threshold.notes}`
      ahu.blocks!.push(structuredClone(error))
      ahu.connections = ahu.connections!.filter(edge => edge !== first && edge !== second)
      ahu.connections.push(
        { from: edgeEndpoints(first)[0], to: `${errorName}.u1` },
        { from: edgeEndpoints(second)[0], to: `${errorName}.u2` },
        { from: `${errorName}.y`, to: `${name}.u` },
      )
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
