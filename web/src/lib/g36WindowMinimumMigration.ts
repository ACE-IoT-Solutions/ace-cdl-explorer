import { edgeEndpoints, layoutFor, type Block, type Project } from './types'

export function needsG36WindowMinimumMigration(project: Project): boolean {
  return project.graph.name === 'FourPipeG36Controls' &&
    (project.graph.blocks ?? []).filter(block => block.block_type === 'VAVBoxController')
      .some(block => block.blocks?.some(child => child.name === 'co2_minimum_flow') &&
        !block.blocks?.some(child => child.name === 'window_primary_minimum'))
}

/** Put the editable window contact between occupied minimum flow and VAV airflow request. */
export function migrateG36WindowMinimum(project: Project, reference: Block): Project {
  if (!needsG36WindowMinimumMigration(project)) return project
  const next = structuredClone(project)
  for (let index = 1; index <= 10; index++) {
    const name = `zone_${String(index).padStart(2, '0')}`
    const zone = next.graph.blocks?.find(block => block.name === name)
    const template = reference.blocks?.find(block => block.name === name)
    const windowBlock = template?.blocks?.find(block => block.name === 'window_primary_minimum')
    if (!zone || !template || !windowBlock) throw new Error(`G36 window minimum reference lacks ${name}`)
    if (zone.blocks?.some(block => block.name === 'window_primary_minimum')) continue
    ;(zone.blocks ??= []).push(structuredClone(windowBlock))
    for (const target of ['airflow_sum.u1', 'airflow_request_sum.u1']) {
      const edge = zone.connections?.find(item => edgeEndpoints(item)[1] === target)
      if (!edge) {
        const output = template.connections?.find(item =>
          edgeEndpoints(item).join('→') === `window_primary_minimum.y→${target}`)
        if (!output) throw new Error(`G36 window minimum reference lacks ${name}.${target}`)
        ;(zone.connections ??= []).push(structuredClone(output))
        continue
      }
      const source = edgeEndpoints(edge)[0]
      if (source === 'co2_minimum_flow.y' || source === 'minimum_airflow_heating.y') {
        edge.from = 'window_primary_minimum.y'
        delete edge.from_block
        delete edge.from_output
      }
    }
    for (const edge of template.connections ?? []) {
      const [from, to] = edgeEndpoints(edge)
      if (to.startsWith('window_primary_minimum.') &&
        !(zone.connections ?? []).some(existing => edgeEndpoints(existing).join('→') === `${from}→${to}`)) {
        (zone.connections ??= []).push(structuredClone(edge))
      }
    }
  }
  next.layout = { ...layoutFor(next.graph), ...project.layout }
  return next
}
