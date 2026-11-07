import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const gates = [
  ['heat_pump_heating', 'heat_pump_heating_kw'],
  ['heat_pump_cooling', 'heat_pump_cooling_kw'],
  ['boiler', 'boiler_kw'],
  ['chiller', 'chiller_kw'],
] as const

/** Move conventional water-source enable decisions into editable CDL switches. */
export function migrateConventionalSourceEnable(saved: Project, reference: Block): Project {
  if (saved.graph.name !== 'FourPipeBuildingControls') return saved
  const existing = saved.graph.blocks?.find(block => block.name === 'plant')
  if (gates.every(([command]) => existing?.blocks?.some(block => block.name === `${command}_enable_gate`))) return saved
  const next = structuredClone(saved)
  const plant = next.graph.blocks?.find(block => block.name === 'plant')
  const template = reference.blocks?.find(block => block.name === 'plant')
  if (!plant || !template) throw new Error('Conventional source-enable migration requires plant')
  for (const [command] of gates) {
    const name = `${command}_enable_gate`
    if (!plant.blocks?.some(block => block.name === name)) {
      const source = template.blocks?.find(block => block.name === name)
      if (!source) throw new Error(`Conventional source-enable reference lacks ${name}`)
      if (!plant.blocks) plant.blocks = []
      plant.blocks.push(structuredClone(source))
    }
  }
  plant.connections = (plant.connections ?? []).filter(edge => {
    const [from, to] = edgeEndpoints(edge)
    return !gates.some(([command, output]) => from === `${command}.y` && to === output)
  })
  for (const edge of template.connections ?? []) {
    const [from, to] = edgeEndpoints(edge)
    if (!from.includes('_enable_gate.') && !to.includes('_enable_gate.')) continue
    if (!plant.connections.some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
      plant.connections.push(structuredClone(edge))
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
