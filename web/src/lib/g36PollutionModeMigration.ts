import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add the 2024 CDL pollution sequence to saved G36 graphs without replacing tuned controls. */
export function needsG36PollutionModeMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeG36Controls') return false
  const mode = saved.graph.blocks?.find(block => block.name === 'outdoor_air_pollution_mode')
  return !mode || !mode.blocks?.some(block => block.name === 'on_elapsed') ||
    !mode.blocks?.some(block => block.name === 'daily_reminder_clock') ||
    [1,2].some(index => !['return_damper_direct_select','economizer_high_limit_lockout','pollution_clear_hold'].every(name =>
      saved.graph.blocks?.find(block => block.name === `ahu_${index}`)?.blocks?.some(block => block.name === name)) ||
      !saved.graph.outputs?.some(port => port.name === `ahu_${index}_minimum_oa_return_direct`) ||
      !saved.graph.connections?.some(edge => edgeEndpoints(edge).join('→') ===
        `ahu_${index}.minimum_oa_return_direct→ahu_${index}_minimum_oa_return_direct`))
}

export function migrateG36PollutionMode(saved: Project, reference: Block): Project {
  if (!needsG36PollutionModeMigration(saved)) return saved
  const next = structuredClone(saved)
  const templateMode = reference.blocks?.find(block => block.name === 'outdoor_air_pollution_mode')
  if (!templateMode) throw new Error('Current G36 example has no outdoor-air pollution sequence')
  next.graph.blocks ??= []
  next.graph.inputs ??= []
  next.graph.outputs ??= []
  next.graph.connections ??= []
  const mode = next.graph.blocks.find(block => block.name === 'outdoor_air_pollution_mode')
  if (!mode) next.graph.blocks.push(structuredClone(templateMode))
  else {
    mode.inputs ??= []; mode.outputs ??= []; mode.blocks ??= []; mode.connections ??= []
    for (const input of templateMode.inputs ?? [])
      if (!mode.inputs.some(port => port.name === input.name)) mode.inputs.push(structuredClone(input))
    for (const output of templateMode.outputs ?? [])
      if (!mode.outputs.some(port => port.name === output.name)) mode.outputs.push(structuredClone(output))
    const replaced = new Set(['on_expired', 'on_not_expired'])
    mode.blocks = mode.blocks.filter(block => !replaced.has(block.name))
    mode.connections = mode.connections.filter(edge => !edgeEndpoints(edge).some(end =>
      [...replaced].some(name => end.startsWith(`${name}.`))))
    for (const block of templateMode.blocks ?? []) {
      if (['on_elapsed', 'on_not_expired', 'daily_reminder_clock', 'reminder_active', 'reminder_level'].includes(block.name) &&
          !mode.blocks.some(current => current.name === block.name)) mode.blocks.push(structuredClone(block))
    }
    const added = new Set(['on_elapsed', 'on_not_expired', 'daily_reminder_clock', 'reminder_active', 'reminder_level',
      'on_duration_s', 'level_three_reminder'])
    const existing = new Set(mode.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of templateMode.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (![from.split('.')[0], to.split('.')[0]].some(name => added.has(name))) continue
      const key = `${from}→${to}`
      if (!existing.has(key)) { mode.connections.push(structuredClone(edge)); existing.add(key) }
    }
    mode.notes = templateMode.notes
  }
  for (const input of reference.inputs ?? []) {
    if (input.name.startsWith('pollution_') && !next.graph.inputs.some(port => port.name === input.name))
      next.graph.inputs.push(structuredClone(input))
  }
  for (const output of reference.outputs ?? []) {
    if ((output.name.startsWith('pollution_') || /^ahu_[12]_minimum_oa_return_direct$/.test(output.name)) &&
        !next.graph.outputs.some(port => port.name === output.name))
      next.graph.outputs.push(structuredClone(output))
  }
  const present = new Set(next.graph.connections.map(edge => edgeEndpoints(edge).join('→')))
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!from.startsWith('pollution_') && !from.startsWith('outdoor_air_pollution_mode.') &&
        !to.startsWith('pollution_') && !to.startsWith('outdoor_air_pollution_mode.') &&
        to !== 'ahu_1.pollution_active' && to !== 'ahu_2.pollution_active' &&
        !/^ahu_[12]_minimum_oa_return_direct$/.test(to)) continue
    const key = `${from}→${to}`
    if (!present.has(key)) { next.graph.connections.push(structuredClone(edge)); present.add(key) }
  }
  for (const index of [1, 2]) {
    const ahu = next.graph.blocks.find(block => block.name === `ahu_${index}`)
    const template = reference.blocks?.find(block => block.name === `ahu_${index}`)
    if (!ahu || !template) throw new Error(`G36 pollution mode requires ahu_${index}`)
    ahu.inputs ??= []
    ahu.outputs ??= []
    ahu.blocks ??= []
    ahu.connections ??= []
    const directOutput = template.outputs?.find(port => port.name === 'minimum_oa_return_direct')
    if (!directOutput) throw new Error(`Current G36 ahu_${index} lacks direct-return output`)
    if (!ahu.outputs.some(port => port.name === directOutput.name)) ahu.outputs.push(structuredClone(directOutput))
    const newInput = template.inputs?.find(port => port.name === 'pollution_active')
    if (!newInput) throw new Error(`Current G36 ahu_${index} lacks pollution input`)
    if (!ahu.inputs.some(port => port.name === newInput.name)) ahu.inputs.push(structuredClone(newInput))
    for (const name of ['pollution_clear', 'pollution_clear_hold', 'economizer_pollution_gate', 'economizer_high_limit_sustained',
      'economizer_high_limit_clear', 'economizer_high_limit_clear_sustained', 'economizer_high_limit_lockout',
      'minimum_oa_direct_request', 'minimum_oa_return_direct_gate', 'return_damper_direct_select']) {
      const block = template.blocks?.find(item => item.name === name)
      if (!block) throw new Error(`Current G36 ahu_${index} lacks ${name}`)
      if (!ahu.blocks.some(item => item.name === name)) ahu.blocks.push(structuredClone(block))
    }
    const former = ahu.connections.filter(edge => edgeEndpoints(edge)[1] === 'economizer_enabled.u2')
    if (former.length !== 1) throw new Error(`Expected one ahu_${index} economizer-enable driver`)
    if (edgeEndpoints(former[0])[0] !== 'economizer_pollution_gate.y')
      former[0].to = 'economizer_pollution_gate.u1'
    const oldPollutionGate = ahu.connections.find(edge => edgeEndpoints(edge).join('→') ===
      'pollution_clear.y→economizer_pollution_gate.u2')
    if (oldPollutionGate) oldPollutionGate.to = 'pollution_clear_hold.u'
    const priorReturn = ahu.connections.filter(edge => edgeEndpoints(edge)[1] === 'return_damper_physical_limit.u1')
    if (priorReturn.length !== 1) throw new Error(`Expected one ahu_${index} return-damper driver`)
    if (edgeEndpoints(priorReturn[0])[0] !== 'return_damper_direct_select.y')
      priorReturn[0].to = 'return_damper_direct_select.u2'
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!['pollution_active', 'pollution_clear.y', 'pollution_clear_hold.y', 'economizer_pollution_gate.y',
        'economizer_dry_bulb_device_select.y', 'economizer_high_limit_clear.y',
        'economizer_high_limit_sustained.y', 'economizer_high_limit_clear_sustained.y',
        'economizer_high_limit_lockout.y',
        'minimum_oa_direct_request.y', 'minimum_oa_mode_enabled.y',
        'minimum_oa_return_direct_gate.y', 'return_damper_pi_max.y',
        'return_damper_mode.y', 'return_damper_direct_select.y'].includes(from)) continue
      if (to === 'return_damper_direct_select.u2' && ahu.connections.some(current => edgeEndpoints(current)[1] === to)) continue
      if (!ahu.connections.some(current => edgeEndpoints(current).join('→') === `${from}→${to}`))
        ahu.connections.push(structuredClone(edge))
    }
  }
  if (next.graph.brick_model && reference.brick_model) {
    const model = next.graph.brick_model
    const ids = new Set(model.entities.map(entity => entity.id))
    for (const entity of reference.brick_model.entities) {
      if (!entity.connector?.startsWith('pollution_') || ids.has(entity.id)) continue
      model.entities.push(structuredClone(entity)); ids.add(entity.id)
    }
    for (const relation of reference.brick_model.relationships) {
      if (!relation.object.startsWith('pollution_') || !ids.has(relation.subject)) continue
      if (!model.relationships.some(current => current.subject === relation.subject &&
          current.predicate === relation.predicate && current.object === relation.object))
        model.relationships.push(structuredClone(relation))
    }
  }
  next.layout = {...layoutFor(next.graph), ...saved.layout}
  return next
}
