import { edgeEndpoints, layoutFor, type Block, type Connection, type Project } from './types'

const added = new Set([
  'economizer_differential_enthalpy_error',
  'economizer_differential_enthalpy_hysteresis',
  'economizer_differential_enthalpy_and_dry_bulb_trip',
  'economizer_use_differential_enthalpy_device',
  'economizer_differential_enthalpy_device_select',
])
const targets = ['economizer_high_limit_hold.u', 'economizer_high_limit_sustained.u',
  'economizer_high_limit_clear.u']

export function needsG36DifferentialEnthalpyMigration(saved: Project): boolean {
  return saved.graph.name === 'FourPipeG36Controls' && [1, 2].some(index =>
    !saved.graph.inputs?.some(port => port.name === `ahu_${index}_return_air_enthalpy_kj_kg`) ||
    !saved.graph.connections?.some(edge => edgeEndpoints(edge).join('→') ===
      `ahu_${index}_return_air_enthalpy_kj_kg→ahu_${index}.return_air_enthalpy_kj_kg`) ||
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)?.inputs
      ?.some(port => port.name === 'return_air_enthalpy_kj_kg') ||
    !saved.graph.blocks?.find(block => block.name === `ahu_${index}`)?.blocks
      ?.some(block => block.name === 'economizer_differential_enthalpy_device_select') ||
    Boolean(saved.graph.brick_model && (
      !saved.graph.brick_model.entities.some(entity =>
        entity.id === `ahu_${index}_return_air_enthalpy_kj_kg`) ||
      !saved.graph.brick_model.relationships.some(relation =>
        relation.object === `ahu_${index}_return_air_enthalpy_kj_kg`))))
}

/** Preserve each prior high-limit driver as the unselected branch. */
export function migrateG36DifferentialEnthalpy(saved: Project, reference: Block): Project {
  if (!needsG36DifferentialEnthalpyMigration(saved)) return saved
  const next = structuredClone(saved)
  next.graph.inputs ??= []
  next.graph.connections ??= []
  for (const index of [1, 2]) {
    const name = `ahu_${index}`
    const point = `${name}_return_air_enthalpy_kj_kg`
    const ahu = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    const input = reference.inputs?.find(port => port.name === point)
    const ahuInput = template?.inputs?.find(port => port.name === 'return_air_enthalpy_kj_kg')
    if (!ahu || !template || !input || !ahuInput) throw new Error(`G36 differential enthalpy migration requires ${name}`)
    ahu.inputs ??= []; ahu.blocks ??= []; ahu.connections ??= []
    if (!next.graph.inputs.some(port => port.name === point)) next.graph.inputs.push(structuredClone(input))
    if (!ahu.inputs.some(port => port.name === 'return_air_enthalpy_kj_kg'))
      ahu.inputs.push(structuredClone(ahuInput))
    const topWire = `${point}→${name}.return_air_enthalpy_kj_kg`
    if (!next.graph.connections.some(edge => edgeEndpoints(edge).join('→') === topWire))
      next.graph.connections.push({ from: point, to: `${name}.return_air_enthalpy_kj_kg` })
    if (reference.brick_model && next.graph.brick_model) {
      const entity = reference.brick_model.entities.find(item => item.id === point)
      const relation = reference.brick_model.relationships.find(item => item.object === point)
      if (entity && !next.graph.brick_model.entities.some(item => item.id === point))
        next.graph.brick_model.entities.push(structuredClone(entity))
      if (relation && !next.graph.brick_model.relationships.some(item => item.object === point))
        next.graph.brick_model.relationships.push(structuredClone(relation))
    }
    if (ahu.blocks.some(block => block.name === 'economizer_differential_enthalpy_device_select')) continue
    const previous = new Map<string, string>()
    for (const target of targets) {
      const wires: Connection[] = ahu.connections.filter((edge: Connection) => edgeEndpoints(edge)[1] === target)
      if (wires.length !== 1) throw new Error(`${name}.${target} needs one prior high-limit driver`)
      previous.set(target, edgeEndpoints(wires[0])[0])
      ahu.connections = ahu.connections.filter((edge: Connection) => edge !== wires[0])
    }
    for (const block of template.blocks ?? [])
      if (added.has(block.name) && !ahu.blocks.some(item => item.name === block.name))
        ahu.blocks.push(structuredClone(block))
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (targets.includes(to) || to === 'economizer_differential_enthalpy_device_select.u3') continue
      if (!added.has(from.split('.')[0]) && !added.has(to.split('.')[0])) continue
      if (!ahu.connections.some(item => edgeEndpoints(item).join('→') === `${from}→${to}`))
        ahu.connections.push(structuredClone(edge))
    }
    const select = template.blocks?.find(block => block.name === 'economizer_differential_enthalpy_device_select')
    if (!select) throw new Error(`Reference ${name} lacks differential device selector`)
    const base = previous.get(targets[0])!
    ahu.connections.push({ from: base, to: 'economizer_differential_enthalpy_device_select.u3' })
    for (const target of targets) {
      const earlier = previous.get(target)!
      if (earlier === base) {
        ahu.connections.push({ from: 'economizer_differential_enthalpy_device_select.y', to: target })
      } else {
        const branch = structuredClone(select)
        branch.name = `${target.split('.')[0]}_differential_enthalpy_select`
        ahu.blocks.push(branch)
        ahu.connections.push(
          { from: 'economizer_differential_enthalpy_and_dry_bulb_trip.y', to: `${branch.name}.u1` },
          { from: 'economizer_use_differential_enthalpy_device.y', to: `${branch.name}.u2` },
          { from: earlier, to: `${branch.name}.u3` },
          { from: `${branch.name}.y`, to: target },
        )
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...saved.layout }
  return next
}
