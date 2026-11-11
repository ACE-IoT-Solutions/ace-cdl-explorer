import { edgeEndpoints, type Block, type Project } from './types'

const retired = new Set([
  'freeze_stage_three_reset_temperature', 'freeze_stage_three_reset_warm',
  'freeze_stage_three_stat_normal', 'freeze_stage_three_reset_safe',
  'freeze_stage_three_reset_allowed',
])

function stockReset(ahu: Block): boolean {
  const connections = ahu.connections ?? []
  const has = (from: string, to: string) => connections.some(edge => {
    const pair = edgeEndpoints(edge)
    return pair[0] === from && pair[1] === to
  })
  return ahu.blocks?.find(block => block.name === 'freeze_stage_three_reset_temperature')
    ?.parameters?.find(parameter => parameter.name === 'k')?.value === 7 &&
    retired.size === [...retired].filter(name => ahu.blocks?.some(block => block.name === name)).length &&
    has('freeze_stage_three_reset_allowed.y', 'freeze_stage_three_latch.clr') &&
    has('freeze_reset', 'freeze_stage_three_reset_allowed.u2')
}

/** Match the pinned OBC software-reset wiring for stock saved AHU graphs. */
export function migrateG36FreezeReset(saved: Project): Project {
  if (!saved.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return saved
  if (!saved.graph.blocks.some(block => block.block_type === 'AHUController' && stockReset(block))) return saved
  const next = structuredClone(saved)
  for (const ahu of next.graph.blocks ?? []) {
    if (ahu.block_type !== 'AHUController' || !stockReset(ahu)) continue
    ahu.blocks = ahu.blocks?.filter(block => !retired.has(block.name))
    ahu.connections = ahu.connections?.filter(edge => {
      const [from, to] = edgeEndpoints(edge)
      return !retired.has(from.split('.')[0]) && !retired.has(to.split('.')[0])
    })
    ahu.connections?.push({ from: 'freeze_reset', to: 'freeze_stage_three_latch.clr' })
    const latch = ahu.blocks?.find(block => block.name === 'freeze_stage_three_latch')
    if (latch) latch.notes = 'Hold freeze shutdown until the operator software reset clears the latch, as in the pinned OBC sequence. A new rising trip retriggers after reset is released.'
  }
  return next
}
