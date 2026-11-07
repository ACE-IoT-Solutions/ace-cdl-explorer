import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

/** Add timed AHU freeze stages and their plant requests to saved G36 graphs. */
export function migrateG36FreezeStageOne(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  if ([1, 2].every(index => saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
    ?.blocks?.some(block => block.name === 'freeze_stage_one_latch')
    && saved.graph.blocks?.find(block => block.name === `ahu_${index}`)
      ?.blocks?.some(block => block.name === 'freeze_stage_two_latch'))
    && saved.graph.blocks?.find(block => block.name === 'plant')?.blocks?.some(block => block.name === 'freeze_hot_water_pump_enable')) return saved
  const next = structuredClone(saved)
  const graph = next.graph
  const addPort = (target: Block, template: Block, direction: 'inputs' | 'outputs', name: string) => {
    const port = template[direction]?.find(item => item.name === name)
    if (!port) throw new Error(`G36 freeze reference lacks ${name}`)
    if (!target[direction]?.some(item => item.name === name)) (target[direction] ??= []).push(structuredClone(port))
  }
  const addEdge = (target: Block, from: string, to: string) => {
    if (!(target.connections ?? []).some(edge => edgeEndpoints(edge).join('→') === `${from}→${to}`)) {
      (target.connections ??= []).push({ from, to })
    }
  }
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`G36 freeze migration requires ${name}`)
    for (const child of template.blocks ?? []) {
      if (child.name.startsWith('freeze_') && !ahu.blocks?.some(block => block.name === child.name)) {
        (ahu.blocks ??= []).push(structuredClone(child))
      }
    }
    for (const portName of ['freeze_stage_one', 'freeze_hot_water_request', 'freeze_stage_two', 'freeze_ventilation_alarm_level']) {
      addPort(ahu, template, 'outputs', portName)
    }
    ahu.connections = (ahu.connections ?? []).filter(edge => ![
      'minimum_oa_command.y→outdoor_air_fraction', 'heating_coil_needed.y→heating_coil_enable.u3',
      'freeze_stage_one_outdoor_air.y→outdoor_air_fraction',
    ].includes(edgeEndpoints(edge).join('→')))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (from.startsWith('freeze_') || to.startsWith('freeze_')) addEdge(ahu, from, to)
    }
    for (const portName of [`${name}_freeze_stage_one`, `${name}_freeze_hot_water_request`,
      `${name}_freeze_stage_two`, `${name}_freeze_ventilation_alarm_level`]) {
      addPort(graph, reference, 'outputs', portName)
    }
  }
  const plant = graph.blocks?.find(block => block.name === 'plant')
  const plantTemplate = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !plantTemplate) throw new Error('G36 freeze migration requires plant')
  for (const index of [1, 2]) addPort(plant, plantTemplate, 'inputs', `ahu_${index}_freeze_hot_water_request`)
  for (const child of plantTemplate.blocks ?? []) {
    if ((child.name.startsWith('freeze_') || child.name === 'hot_reset_with_freeze_requests')
      && !plant.blocks?.some(block => block.name === child.name)) (plant.blocks ??= []).push(structuredClone(child))
  }
  plant.connections = (plant.connections ?? []).filter(edge => ![
    'hot_reset_all_request_count.y→hot_reset_request_real.u',
    'hot_water_pump_needed.y→hot_water_enable',
    'hot_water_pump_needed.y→hot_water_pump_select.u3',
    'hot_water_pump_needed.y→hot_reset_enabled.u3',
    'hot_reset_supply_sum.y→hot_water_supply_setpoint_c',
  ].includes(edgeEndpoints(edge).join('→')))
  for (const edge of plantTemplate.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (from.startsWith('freeze_') || to.startsWith('freeze_') || from.startsWith('hot_reset_with_freeze_') || to.startsWith('hot_reset_with_freeze_')) {
      addEdge(plant, from, to)
    }
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (from.includes('freeze_') || to.includes('freeze_')) addEdge(graph, from, to)
  }
  next.layout = { ...layoutFor(graph), ...saved.layout }
  return next
}
