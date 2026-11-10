import { edgeEndpoints, type Block, type Project } from './types'

const addedBlocks = new Set([
  'minimum_oa_design_flow', 'minimum_oa_design_floor', 'minimum_oa_design_safe',
  'minimum_oa_setpoint_normalized', 'minimum_oa_measured_normalized',
])

function hasRawLoop(ahu: Block): boolean {
  if (ahu.block_type !== 'AHUController') return false
  const pi = ahu.blocks?.find(block => block.name === 'minimum_oa_pi')
  if (!pi || !pi.block_type.endsWith('LimPID')) return false
  const edges = new Set((ahu.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))
  return edges.has('minimum_oa_setpoint.y→minimum_oa_pi.u_s')
    && edges.has('outdoor_airflow→minimum_oa_pi.u_m')
}

function hasGatedNormalizedLoop(ahu: Block): boolean {
  if (ahu.block_type !== 'AHUController') return false
  const pi = ahu.blocks?.find(block => block.name === 'minimum_oa_pi')
  if (!pi?.block_type.endsWith('PIDWithReset')) return false
  const edges = new Set((ahu.connections ?? []).map(edge => edgeEndpoints(edge).join('→')))
  return edges.has('minimum_oa_setpoint.y→minimum_oa_setpoint_normalized.u1')
    && edges.has('minimum_oa_setpoint_normalized.y→minimum_oa_pi.u_s')
    && ahu.blocks?.some(block => block.name === 'vent_effective_limit') === true
}

/** Move saved common-damper loops to design-normalized feedback and a fan-proof reset. */
export function needsG36MinimumOutdoorAirPiMigration(project: Project): boolean {
  if (project.graph.name !== 'FourPipeG36Controls'
    && !project.graph.blocks?.some(block => block.block_type === 'ZoneGroupModeController')) return false
  return (project.graph.blocks ?? []).some(ahu => hasRawLoop(ahu) || hasGatedNormalizedLoop(ahu))
}

export function migrateG36MinimumOutdoorAirPi(project: Project, reference: Block): Project {
  if (!needsG36MinimumOutdoorAirPiMigration(project)) return project
  const next = structuredClone(project)
  for (const ahu of next.graph.blocks ?? []) {
    if (hasGatedNormalizedLoop(ahu)) {
      const gated = ahu.connections?.find(edge => edgeEndpoints(edge).join('→') === 'minimum_oa_setpoint.y→minimum_oa_setpoint_normalized.u1')
      if (gated) gated.from = 'vent_effective_limit.y'
      continue
    }
    if (!hasRawLoop(ahu)) continue
    const template = reference.blocks?.find(block => block.name === ahu.name)
    const oldPi = ahu.blocks?.find(block => block.name === 'minimum_oa_pi')
    const newPi = template?.blocks?.find(block => block.name === 'minimum_oa_pi')
    if (!template || !oldPi || !newPi) throw new Error(`G36 minimum outdoor-air PI reference lacks ${ahu.name}`)
    const oldParams = new Map(oldPi.parameters?.map(parameter => [parameter.name, parameter]))
    const oldNotes = oldPi.notes
    oldPi.block_type = newPi.block_type
    oldPi.inputs = structuredClone(newPi.inputs)
    oldPi.parameters = structuredClone(newPi.parameters)
    const set = (name: string, value: number | boolean) => {
      const parameter = oldPi.parameters?.find(item => item.name === name)
      if (parameter) parameter.value = value
    }
    const design = Number(template.blocks?.find(block => block.name === 'minimum_oa_design_flow')?.parameters?.find(parameter => parameter.name === 'k')?.value ?? 0.75)
    const oldGain = oldParams.get('k')?.value
    if (typeof oldGain === 'number' && oldGain !== 0.2) set('k', oldGain * design)
    const oldTi = oldParams.get('Ti')?.value
    if (typeof oldTi === 'number' && oldTi !== 3600) set('Ti', oldTi)
    for (const name of ['yMin', 'yMax', 'reverseActing'] as const) {
      const oldValue = oldParams.get(name)?.value
      if (typeof oldValue === 'number' || typeof oldValue === 'boolean') set(name, oldValue)
    }
    if (oldNotes && oldNotes !== 'Increase outside-air damper command when measured flow misses its minimum setpoint.') oldPi.notes = oldNotes
    else oldPi.notes = newPi.notes
    for (const block of template.blocks ?? []) {
      if (addedBlocks.has(block.name) && !ahu.blocks?.some(existing => existing.name === block.name)) {
        (ahu.blocks ??= []).push(structuredClone(block))
      }
    }
    ahu.connections = (ahu.connections ?? []).filter(edge => {
      const [, to] = edgeEndpoints(edge)
      return to !== 'minimum_oa_pi.u_s' && to !== 'minimum_oa_pi.u_m'
    })
    const existing = new Set(ahu.connections.map(edge => edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!addedBlocks.has(from.split('.')[0]) && !addedBlocks.has(to.split('.')[0])
        && to !== 'minimum_oa_pi.trigger') continue
      const key = `${from}→${to}`
      if (!existing.has(key)) { ahu.connections.push(structuredClone(edge)); existing.add(key) }
    }
  }
  return next
}
