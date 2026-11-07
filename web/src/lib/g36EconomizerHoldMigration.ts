import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export function needsG36EconomizerHoldMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'economizer_high_limit_hold'))
}

/** Insert the reference high-limit hold without replacing edited AHU controls. */
export function migrateG36EconomizerHold(saved: Project, reference: Block): Project {
  if (!needsG36EconomizerHoldMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    const hold = template?.blocks?.find(block => block.name === 'economizer_high_limit_hold')
    if (!ahu || !template || !hold) throw new Error(`G36 economizer hold migration requires ${name}`)
    ahu.blocks ??= []
    ahu.connections ??= []
    if (ahu.blocks.some(block => block.name === hold.name)) continue
    ahu.blocks.push(structuredClone(hold))
    const old = ahu.connections.find(edge => {
      const [from, to] = edgeEndpoints(edge)
      return from === 'economizer_high_limit_hysteresis.y' && to === 'economizer_high_limit_ok.u'
    })
    if (!old) throw new Error(`G36 economizer hold migration requires ${name} high-limit wire`)
    if ('to' in old) old.to = 'economizer_high_limit_hold.u'
    else { old.to_block = 'economizer_high_limit_hold'; old.to_input = 'u' }
    const edge = template.connections?.find(item => {
      const [from, to] = edgeEndpoints(item)
      return from === 'economizer_high_limit_hold.y' && to === 'economizer_high_limit_ok.u'
    })
    if (!edge) throw new Error(`G36 economizer hold migration requires ${name} output wire`)
    ahu.connections.push(structuredClone(edge))
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
