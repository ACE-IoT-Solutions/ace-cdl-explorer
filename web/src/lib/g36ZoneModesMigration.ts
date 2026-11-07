import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const names = new Set([
  'warmup_error', 'cooldown_error', 'setback_error', 'setup_error',
  'warmup_needed', 'cooldown_needed', 'setback_needed', 'setup_needed',
  'warmup_mode', 'cooldown_mode', 'preconditioning_needed',
  'occupied_setpoint_requested',
  'mode_unoccupied', 'mode_occupied', 'mode_standby', 'mode_warmup',
  'mode_cooldown', 'mode_setup', 'mode_setback',
  'mode_select_setback', 'mode_select_setup', 'mode_select_cooldown',
  'mode_select_warmup', 'mode_select_occupied', 'mode_select_schedule',
])
const rewired = new Set(['conditioning_mode.u2', 'heating_warmup_airflow.u3',
  'heat_setpoint_standby.u3', 'cool_setpoint_standby.u3'])

/** Install local CDL thermal modes while preserving every saved zone parameter. */
export function migrateG36ZoneModes(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if (saved.graph.blocks?.find(block => block.name === 'zone_01')?.blocks?.some(block => block.name === 'mode_select_schedule')) return saved
  const next = structuredClone(saved)
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!zone || !template) throw new Error(`G36 zone mode migration requires ${name}`)
    next.graph.outputs = [...(next.graph.outputs ?? []), structuredClone(reference.outputs!.find(port => port.name === `${name}_conditioning_state`)!)]
    next.graph.connections = [...(next.graph.connections ?? []),
      structuredClone(reference.connections!.find(edge => edgeEndpoints(edge)[1] === `${name}_conditioning_state`)!)]
    zone.outputs = [...(zone.outputs ?? []), structuredClone(template.outputs!.find(port => port.name === 'conditioning_state')!)]
    for (const block of template.blocks ?? []) {
      if (names.has(block.name)) zone.blocks = [...(zone.blocks ?? []), structuredClone(block)]
    }
    zone.connections = (zone.connections ?? []).filter(edge => !rewired.has(edgeEndpoints(edge)[1]))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (!names.has(from.split('.')[0]) && !names.has(to.split('.')[0]) &&
        !rewired.has(to) && to !== 'conditioning_state') continue
      if (!zone.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
        zone.connections.push(structuredClone(edge))
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
