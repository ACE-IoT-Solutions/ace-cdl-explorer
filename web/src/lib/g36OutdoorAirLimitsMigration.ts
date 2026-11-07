import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const addedBlocks = new Set([
  'minimum_oa_mode_enabled', 'minimum_oa_off_setpoint', 'minimum_oa_setpoint',
  'minimum_oa_off_command', 'minimum_oa_active', 'outdoor_air_fan_off',
  'outdoor_air_fan_requested', 'outdoor_damper_physical_max', 'outdoor_damper_physical_limit',
])

/** Add occupied/fan minimum-air gating and physical outdoor-damper travel to saved G36 graphs. */
export function migrateG36OutdoorAirLimits(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if ([1, 2].every(index => saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
    ?.blocks?.some(block => block.name === 'outdoor_damper_physical_limit'))
    && saved.graph.outputs?.some(port => port.name === 'ahu_1_minimum_outdoor_air_enabled')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const addPort = (target: Block, template: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = template[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`G36 outdoor-air reference lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  const obsolete = new Set([
    'minimum_oa_flow_gain.y→minimum_outdoor_airflow',
    'minimum_oa_flow_gain.y→minimum_oa_pi.u_s',
    'minimum_oa_pi.y→minimum_oa_command.u2',
    'minimum_oa_command.y→freeze_stage_one_outdoor_air.u2',
    'freeze_stage_three_outdoor_air.y→outdoor_air_fraction',
  ])
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 outdoor-air migration requires ${name}`)
    addPort(ahu, template, 'inputs', 'occupied_mode')
    addPort(ahu, template, 'outputs', 'minimum_outdoor_air_enabled')
    addPort(graph, reference, 'outputs', `${name}_minimum_outdoor_air_enabled`)
    for (const child of template.blocks ?? []) {
      if (addedBlocks.has(child.name) && !ahu.blocks?.some(block => block.name === child.name)) {
        (ahu.blocks ??= []).push(structuredClone(child))
      }
    }
    ahu.connections = (ahu.connections ?? []).filter(edge => !obsolete.has(edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to === 'outdoor_air_damper_position') continue
      if (addedBlocks.has(from.split('.')[0]) || addedBlocks.has(to.split('.')[0])
        || from === 'occupied_mode' || to === 'minimum_outdoor_air_enabled') addEdge(ahu, from, to)
    }
    addEdge(graph, 'schedule_occupied', `${name}.occupied_mode`)
    addEdge(graph, `${name}.minimum_outdoor_air_enabled`, `${name}_minimum_outdoor_air_enabled`)
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
