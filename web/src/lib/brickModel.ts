import type { Block, BrickModel, Project } from './types'

const BASE = 'https://ace-iot-solutions.github.io/ace-cdl-explorer/building#'

export function migrateBrickModel(saved: Project, reference: Block): Project {
  if (saved.graph.brick_model || !reference.brick_model) return saved
  const next = structuredClone(saved)
  next.graph.brick_model = structuredClone(reference.brick_model)
  return next
}

export function validateBrickModel(graph: Block): string[] {
  const model = graph.brick_model
  if (!model) return []
  const errors: string[] = []
  const ids = new Set<string>()
  for (const entity of model.entities) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(entity.id) || ids.has(entity.id))
      errors.push(`Invalid or duplicate Brick entity ${entity.id}`)
    ids.add(entity.id)
    if (entity.controller && !graph.blocks?.some(block => block.name === entity.controller))
      errors.push(`Brick entity ${entity.id} refers to missing controller ${entity.controller}`)
    if (entity.connector && !graph.inputs?.some(port => port.name === entity.connector))
      errors.push(`Brick entity ${entity.id} refers to missing input ${entity.connector}`)
  }
  for (const relation of model.relationships) {
    if (!ids.has(relation.subject) || !ids.has(relation.object))
      errors.push(`Brick relation ${relation.subject} ${relation.predicate} ${relation.object} has a missing entity`)
  }
  for (const zone of model.entities.filter(entity => entity.class === 'brick:HVAC_Zone')) {
    const rooms = model.relationships.filter(relation => relation.subject === zone.id &&
      relation.predicate === 'brick:hasPart' && model.entities.some(entity => entity.id === relation.object && entity.class === 'rec:Room'))
    if (rooms.length !== 1) errors.push(`${zone.id} must map to one modeled room`)
  }
  return errors
}

/** Independent Brick/REC model; CDL CXF remains a control-sequence document. */
export function exportBrickModel(model: BrickModel): Record<string, unknown> {
  const bySubject = new Map<string, Array<{ predicate: string; object: string }>>()
  for (const relation of model.relationships) {
    const current = bySubject.get(relation.subject) ?? []
    current.push(relation)
    bySubject.set(relation.subject, current)
  }
  return {
    '@context': {
      brick: 'https://brickschema.org/schema/Brick#',
      rec: 'https://w3id.org/rec#',
      rdfs: 'http://www.w3.org/2000/01/rdf-schema#',
      ace: BASE,
    },
    '@graph': model.entities.map(entity => {
      const node: Record<string, unknown> = {
        '@id': `${BASE}${entity.id}`, '@type': entity.class, 'rdfs:label': entity.label,
      }
      if (entity.controller) node['ace:controllerPath'] = entity.controller
      if (entity.connector) node['ace:connector'] = entity.connector
      for (const relation of bySubject.get(entity.id) ?? []) {
        const targets = node[relation.predicate] as Array<{ '@id': string }> | undefined
        node[relation.predicate] = [...(targets ?? []), { '@id': `${BASE}${relation.object}` }]
      }
      return node
    }),
  }
}
