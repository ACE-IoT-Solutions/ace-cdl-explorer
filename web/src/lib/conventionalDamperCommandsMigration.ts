import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

const commands = ['outdoor_air_damper_position', 'return_air_damper_position'] as const

export function needsConventionalDamperCommandsMigration(saved: Project): boolean {
  if (saved.graph.name !== 'FourPipeBuildingControls') return false
  return [1, 2].some(index => commands.some(command =>
    !saved.graph.outputs?.some(port => port.name === `ahu_${index}_${command}`)))
}

/** Put both physical AHU damper commands in the conventional CDL graph. */
export function migrateConventionalDamperCommands(saved: Project, reference: Block): Project {
  if (!needsConventionalDamperCommandsMigration(saved)) return saved
  const next = structuredClone(saved)
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    if (!ahu || !template) throw new Error(`Conventional damper migration requires ${name}`)
    ahu.blocks ??= []
    ahu.outputs ??= []
    ahu.connections ??= []
    next.graph.outputs ??= []
    next.graph.connections ??= []
    const outdoorSource = ahu.connections.find(edge => edgeEndpoints(edge)[1] === 'outdoor_air_fraction')
    if (!outdoorSource) throw new Error(`${name} is missing its CDL outdoor-air command`)
    const outdoor = edgeEndpoints(outdoorSource)[0]
    for (const blockName of ['conventional_return_one', 'conventional_return_complement']) {
      if (ahu.blocks.some(block => block.name === blockName)) continue
      const block = template.blocks?.find(block => block.name === blockName)
      if (!block) throw new Error(`Conventional damper reference lacks ${blockName}`)
      ahu.blocks.push(structuredClone(block))
    }
    for (const command of commands) {
      if (!ahu.outputs.some(port => port.name === command)) {
        const port = template.outputs?.find(port => port.name === command)
        if (!port) throw new Error(`Conventional damper reference lacks ${command}`)
        ahu.outputs.push(structuredClone(port))
      }
      const rootName = `${name}_${command}`
      if (!next.graph.outputs.some(port => port.name === rootName)) {
        const port = reference.outputs?.find(port => port.name === rootName)
        if (!port) throw new Error(`Conventional damper reference lacks ${rootName}`)
        next.graph.outputs.push(structuredClone(port))
      }
      if (!next.graph.connections.some(edge => edgeEndpoints(edge)[1] === rootName)) {
        next.graph.connections.push({ from: `${name}.${command}`, to: rootName })
      }
    }
    const wires = [
      [outdoor, 'outdoor_air_damper_position'],
      ['conventional_return_one.y', 'conventional_return_complement.u1'],
      [outdoor, 'conventional_return_complement.u2'],
      ['conventional_return_complement.y', 'return_air_damper_position'],
    ]
    for (const [from, to] of wires) {
      ahu.connections = ahu.connections.filter(edge => edgeEndpoints(edge)[1] !== to)
      ahu.connections.push({ from, to })
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
