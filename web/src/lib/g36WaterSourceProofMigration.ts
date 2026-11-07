import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const commands = ['heat_pump_heating', 'boiler', 'heat_pump_cooling', 'chiller']
const outputs = ['heat_pump_heating_kw', 'boiler_kw', 'heat_pump_cooling_kw', 'chiller_kw']
const isProofBlock = (name: string) => name.startsWith('hot_source_') || name.startsWith('chilled_source_') || name.endsWith('_flow_gate')

/** Prove measured loop flow in CDL before enabling water-source equipment. */
export function migrateG36WaterSourceProof(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeG36Controls') return saved
  const existing = saved.graph.blocks?.find(block => block.name === 'plant')
  if (existing?.blocks?.some(block => block.name === 'hot_source_proven')
    && existing.blocks.some(block => block.name === 'chilled_source_proven')) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('G36 water-source proof migration requires plant')
  for (const child of template.blocks ?? []) {
    if (isProofBlock(child.name) && !plant.blocks?.some(block => block.name === child.name)) {
      (plant.blocks ??= []).push(structuredClone(child))
    }
  }
  for (const name of ['hot_source_proven_output', 'chilled_source_proven_output']) {
    const port = template.outputs?.find(port => port.name === name)
    if (!port) throw new Error(`G36 source-proof reference lacks ${name}`)
    if (!plant.outputs?.some(item => item.name === name)) (plant.outputs ??= []).push(structuredClone(port))
  }
  for (const name of ['plant_hot_source_proven', 'plant_chilled_source_proven']) {
    const port = reference.outputs?.find(port => port.name === name)
    if (!port) throw new Error(`G36 source-proof reference lacks ${name}`)
    if (!next.graph.outputs?.some(item => item.name === name)) (next.graph.outputs ??= []).push(structuredClone(port))
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !commands.some((command, index) => from === `${command}.y` && to === outputs[index])
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (isProofBlock(from.split('.')[0]) || isProofBlock(to.split('.')[0])
      || to.endsWith('_source_proven_output')) {
      if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
        plant.connections.push({ from, to })
      }
    }
  }
  for (const edge of reference.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (to === 'plant_hot_source_proven' || to === 'plant_chilled_source_proven') {
      if (!(next.graph.connections ?? []).some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
        (next.graph.connections ??= []).push({ from, to })
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
