import type { Block } from './types'

export function hasLegacyVavLeaf(block: Block): boolean {
  return (block.block_type === 'VAVBoxController' && !block.blocks?.length)
    || (block.blocks?.some(hasLegacyVavLeaf) ?? false)
}

/** Expand python-cdl's former VAV shortcut into the editable CDL composite. */
export function expandLegacyVavLeaves(graph: Block, template: Block): Block {
  function expand(block: Block): Block {
    if (block.block_type !== 'VAVBoxController' || block.blocks?.length) {
      return { ...block, blocks: block.blocks?.map(expand) }
    }
    const next = structuredClone(template)
    next.name = block.name
    next.description = block.description ?? next.description
    next.notes = block.notes ?? 'Imported VAV values, represented as editable CDL blocks.'
    next.brick_model = block.brick_model
    const children = new Map((next.blocks ?? []).map(child => [child.name, child]))
    const original = new Map((block.parameters ?? []).map(parameter => [parameter.name, parameter.value]))
    const value = (name: string, fallback: number): number => {
      const input = original.get(name)
      return typeof input === 'number' && Number.isFinite(input) ? input : fallback
    }
    const set = (childName: string, parameterName: string, number: number) => {
      const parameter = children.get(childName)?.parameters?.find(item => item.name === parameterName)
      if (parameter) parameter.value = number
    }
    const minimumFlow = value('min_airflow', 0.2)
    const maximumFlow = value('max_airflow', 0.95)
    const minimumDamper = value('min_damper_position', 0.15)
    const coolingK = value('kp_damper', 0.6)
    const coolingKi = value('ki_damper', 0.12)
    const heatingK = value('kp_reheat', 0.35)
    const heatingKi = value('ki_reheat', 0.06)
    set('cool_setpoint', 'k', value('cooling_setpoint', 24))
    set('heat_setpoint', 'k', value('heating_setpoint', 21))
    set('minimum_airflow', 'k', minimumFlow)
    set('minimum_damper', 'k', minimumDamper)
    set('flow_span', 'k', maximumFlow - minimumFlow)
    set('cool_pi', 'k', coolingK)
    if (coolingKi > 0) set('cool_pi', 'Ti', 900 * coolingK / coolingKi)
    set('cool_pi', 'yMax', 1 - minimumDamper)
    set('heat_pi', 'k', heatingK)
    if (heatingKi > 0) set('heat_pi', 'Ti', 900 * heatingK / heatingKi)
    set('damper_limit', 'uMin', minimumDamper)
    return next
  }
  return expand(graph)
}
