import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const runningNames = ['heat_pump_running', 'boiler_running', 'chiller_running']
const suffixes = [
  'hp_confirmed', 'backup_confirmed', 'hp_not_requested', 'backup_not_requested',
  'hp_ready', 'backup_ready', 'sources_ready', 'ready_integer', 'ready_real',
  'ready_observed', 'ready_threshold', 'ready_confirmed', 'ready_and_flow',
]
const addedNames = new Set(['hot', 'chilled'].flatMap(loop => suffixes.map(suffix => `${loop}_stage_${suffix}`)))
const replacedTargets = new Set(['hot', 'chilled'].flatMap(loop => [
  `${loop}_stage_hp_select.u3`, `${loop}_stage_backup_select.u3`, `${loop}_stage_hold_delay.u`,
]))

/** Connect physical running proof to saved G36 water reset holds. */
export function migrateG36StageFeedback(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const previous = saved.graph.blocks?.find(block => block.name === 'plant')
  if (previous?.blocks?.some(block => block.name === 'hot_stage_ready_observed')) return saved
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!previous || !template) throw new Error('G36 stage feedback migration requires plant')
  const next = structuredClone(saved)
  const plant = next.graph.blocks!.find(block => block.name === 'plant')!
  for (const name of runningNames) {
    if (!next.graph.inputs?.some(port => port.name === name)) {
      next.graph.inputs = [...(next.graph.inputs ?? []), structuredClone(reference.inputs!.find(port => port.name === name)!)]
    }
    if (!plant.inputs?.some(port => port.name === name)) {
      plant.inputs = [...(plant.inputs ?? []), structuredClone(template.inputs!.find(port => port.name === name)!)]
    }
    if (!next.graph.connections?.some(edge => edgeEndpoints(edge).join('→') === `${name}→plant.${name}`)) {
      next.graph.connections = [...(next.graph.connections ?? []), { from: name, to: `plant.${name}` }]
    }
  }
  for (const block of template.blocks ?? []) {
    if (addedNames.has(block.name) && !plant.blocks?.some(existing => existing.name === block.name)) {
      plant.blocks = [...(plant.blocks ?? []), structuredClone(block)]
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => !replacedTargets.has(edgeEndpoints(edge)[1]))
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!addedNames.has(from.split('.')[0]) && !addedNames.has(to.split('.')[0]) && !replacedTargets.has(to)) continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
