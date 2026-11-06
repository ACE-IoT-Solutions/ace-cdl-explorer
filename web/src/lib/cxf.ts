import type { Block, Connection, Connector, Parameter } from './types'
import { edgeEndpoints } from './types'

// CXF is JSON-LD. The S231 names and instance paths follow the OBC CXF specification.
// The official CXF-Core document supplies interfaces for referenced CDL classes.
type RecordNode = Record<string, unknown> & { '@id': string; '@type'?: string }
export interface CxfDocument { '@context': Record<string, string>; '@graph': RecordNode[] }
const S = 'S231:'
const BASE = 'https://ace-iot-solutions.github.io/ace-cdl-explorer/cxf#'
const CONTEXT = { S231: 'http://data.ashrae.org/S231#', qudt: 'http://qudt.org/schema/qudt#', unit: 'http://qudt.org/vocab/unit#', ace: BASE }
const asList = (value: unknown): RecordNode[] => value == null ? [] : Array.isArray(value) ? value as RecordNode[] : [value as RecordNode]
const refs = (value: unknown): string[] => asList(value).map(item => item?.['@id']).filter((id): id is string => typeof id === 'string')
const shortId = (id: string) => id.slice(id.lastIndexOf('#') + 1)
const lastName = (id: string) => id.slice(Math.max(id.lastIndexOf('.'), id.lastIndexOf('#')) + 1)
const plain = (value: unknown): string | number | boolean | undefined => {
  const raw = value && typeof value === 'object' ? (value as Record<string, unknown>)['@value'] : value
  if (typeof raw === 'number' || typeof raw === 'boolean') return raw
  if (typeof raw === 'string') {
    if (/^(true|false)$/.test(raw)) return raw === 'true'
    if (/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.test(raw)) return Number(raw)
    return raw
  }
  return undefined
}
const ref = (id: string) => ({ '@id': id })
const dataType = (type: unknown): Connector['type'] => {
  const value = Array.isArray(type) ? type[0] : type
  const name = typeof value === 'string' ? value : ''
  if (name.includes('Boolean')) return 'Boolean'
  if (name.includes('Integer')) return 'Integer'
  if (name.includes('String')) return 'String'
  return 'Real'
}
const classId = (id: string) => id.replace(/^http:\/\/example\.org#/, 'http://data.ashrae.org/S231#')
const unitName = (node: RecordNode): string | undefined => {
  const id = refs(node['S231:hasDisplayUnit'])[0] ?? refs(node['qudt:hasUnit'])[0]
  if (!id) return undefined
  const name = lastName(id).split(':').at(-1) ?? ''
  return ({ DEG_C: '°C', K: 'K', SEC: 's', W: 'W', KiloW: 'kW', PA: 'Pa', PERCENT: '%' } as Record<string, string>)[name] ?? name
}
const unitId = (unit: string) => ({ '°C': 'unit:DEG_C', K: 'unit:K', s: 'unit:SEC', W: 'unit:W', kW: 'unit:KiloW', Pa: 'unit:PA', '%': 'unit:PERCENT' } as Record<string, string>)[unit]

export function cxfIndex(document: CxfDocument): Map<string, RecordNode> {
  if (!document || !Array.isArray(document['@graph'])) throw new Error('Expected a CXF JSON-LD @graph')
  return new Map(document['@graph'].filter(node => typeof node?.['@id'] === 'string').map(node => [node['@id'], node]))
}

export function importCxf(document: CxfDocument, core: CxfDocument): Block {
  const nodes = cxfIndex(document)
  const library = cxfIndex(core)
  const definition = (id: string) => nodes.get(id) ?? library.get(id) ?? library.get(classId(id))
  const root = document['@graph'].find(node => ['S231:Block', 'S231:CompositeBlock', 'S231:ElementaryBlock', 'S231:ExtensionBlock'].includes(String(node['@type'])) && (node['S231:containsBlock'] || node['S231:hasInput'] || node['S231:hasOutput']))
  if (!root) throw new Error('CXF has no top-level CDL block')
  const rootId = root['@id']
  const ports = new Map<string, 'input' | 'output'>()
  const connector = (id: string, owner: string, direction: 'input' | 'output'): Connector => {
    const node = definition(id) ?? { '@id': id }
    const local = `${owner}.${lastName(id)}`
    const instance = nodes.get(local)
    const type = dataType(instance?.['@type'] ?? node['@type'])
    ports.set(local, direction)
    const result: Connector = { name: lastName(id), type }
    const brickClass=plain(instance?.['ace:brickClass'] ?? node['ace:brickClass'])
    const pointLabel=plain(instance?.['ace:pointLabel'] ?? node['ace:pointLabel'])
    if(typeof brickClass==='string')result.brick_class=brickClass
    if(typeof pointLabel==='string')result.label=pointLabel
    const description = plain(instance?.['S231:description'] ?? node['S231:description'])
    if (typeof description === 'string') result.description = description
    const unit = instance && unitName(instance) || unitName(node)
    const localUnit=plain(instance?.['ace:unit'] ?? node['ace:unit'])
    if (typeof localUnit==='string') result.unit=localUnit
    else if (unit) result.unit = unit
    const baseUnit = refs(instance?.['qudt:hasUnit'] ?? node['qudt:hasUnit'])[0]
    const displayUnit = refs(instance?.['S231:hasDisplayUnit'] ?? node['S231:hasDisplayUnit'])[0]
    if (baseUnit) result.cxf_unit = baseUnit
    if (displayUnit) result.cxf_display_unit = displayUnit
    return result
  }
  const parameter = (id: string, owner: string): Parameter => {
    const node = definition(id) ?? { '@id': id }
    const instance = nodes.get(`${owner}.${lastName(id)}`)
    const raw = plain(instance?.['S231:value'] ?? node['S231:value'])
    const value = raw ?? ''
    const result: Parameter = { name: lastName(id), value, type: dataType(refs(node['S231:isOfDataType'])[0] ?? node['@type']) }
    const unit = instance && unitName(instance) || unitName(node)
    if (unit) result.unit = unit
    const baseUnit = refs(instance?.['qudt:hasUnit'] ?? node['qudt:hasUnit'])[0]
    const displayUnit = refs(instance?.['S231:hasDisplayUnit'] ?? node['S231:hasDisplayUnit'])[0]
    if (baseUnit) result.cxf_unit = baseUnit
    if (displayUnit) result.cxf_display_unit = displayUnit
    return result
  }
  const seen = new Set<string>()
  const visit = (node: RecordNode, id: string, top = false): Block => {
    if (seen.has(id)) throw new Error(`CXF contains a cycle at ${id}`)
    seen.add(id)
    const typeId = String(node['@type'] ?? 'Composite')
    const classNode = top ? node : definition(typeId)
    const standardType = shortId(top ? id : typeId)
    const type = String(plain(node['ace:blockType']) ?? (standardType.startsWith('Buildings.') ? standardType : top ? 'Composite' : plain(classNode?.['S231:label']) ?? standardType))
    const inputIds = refs(top ? node['S231:hasInput'] : classNode?.['S231:hasInput'])
    const outputIds = refs(top ? node['S231:hasOutput'] : classNode?.['S231:hasOutput'])
    const parameterIds = refs(top ? node['S231:hasParameter'] : classNode?.['S231:hasParameter'])
    // Imported instance records may contain ports absent from the class library.
    const instanceIds = refs(node['S231:hasInstance'])
    for (const child of instanceIds) {
      const suffix = lastName(child)
      if (inputIds.some(item => lastName(item) === suffix) || outputIds.some(item => lastName(item) === suffix) || parameterIds.some(item => lastName(item) === suffix)) continue
      const item = nodes.get(child)
      if (!item) continue
      const kind = String(item['@type'] ?? '')
      if (kind.endsWith('Input')) inputIds.push(child)
      else if (kind.endsWith('Output')) outputIds.push(child)
      else if (kind.endsWith('Parameter')) parameterIds.push(child)
    }
    const block: Block = {
      name: top ? String(plain(node['S231:label']) ?? lastName(id)) : lastName(id),
      block_type: type,
      inputs: inputIds.map(port => connector(port, id, 'input')),
      outputs: outputIds.map(port => connector(port, id, 'output')),
      parameters: parameterIds.map(item => parameter(item, id)),
    }
    const description = plain(node['S231:description'])
    if (typeof description === 'string') block.description = description
    const notes = plain(node['ace:notes'])
    if (typeof notes === 'string') block.notes = notes
    const childIds = refs(node['S231:containsBlock'])
    if (childIds.length) {
      block.category = 'composite'
      block.blocks = childIds.map(child => {
        const childNode = nodes.get(child)
        if (!childNode) throw new Error(`CXF is missing block instance ${child}`)
        return visit(childNode, child)
      })
    }
    return block
  }
  const graph = visit(root, rootId, true)
  const blockById = new Map<string, Block>()
  const register = (block: Block, id: string) => { blockById.set(id, block); for (const child of block.blocks ?? []) register(child, `${id}.${child.name}`) }
  register(graph, rootId)
  for (const [id, block] of blockById) {
    if (!block.blocks?.length) continue
    const connections: Connection[] = []
    const added = new Set<string>()
    for (const node of document['@graph']) {
      const left = node['@id']
      if (!left.startsWith(`${id}.`) || !ports.has(left)) continue
      for (const right of [...refs(node['S231:isConnectedTo']), ...refs(node['S231:connectedTo'])]) {
        if (!right.startsWith(`${id}.`) || !ports.has(right)) continue
        const a = left.slice(id.length + 1), b = right.slice(id.length + 1)
        if (a.split('.').length > 2 || b.split('.').length > 2) continue
        const isDriver = (endpoint: string) => endpoint.slice(id.length + 1).includes('.') ? ports.get(endpoint) === 'output' : ports.get(endpoint) === 'input'
        const source = isDriver(left) && !isDriver(right) ? a : isDriver(right) && !isDriver(left) ? b : a
        const target = source === a ? b : a
        const key = `${source}>${target}`
        if (!added.has(key)) { connections.push({ from: source, to: target }); added.add(key) }
      }
    }
    block.connections = connections
  }
  return graph
}

export function exportCxf(graph: Block, core: CxfDocument): CxfDocument {
  const library = cxfIndex(core)
  const nodes: RecordNode[] = []
  const rootId = `${BASE}${encodeURIComponent(graph.name)}`
  const knownClass = (type: string): string | undefined => {
    const name = type.startsWith('CDL.') ? `Buildings.Controls.OBC.${type}` : type
    const id = `http://data.ashrae.org/S231#${name}`
    return library.has(id) ? id : undefined
  }
  const addPorts = (block: Block, id: string, node: RecordNode) => {
    const instances: ReturnType<typeof ref>[] = []
    for (const [key, direction] of [['inputs', 'Input'], ['outputs', 'Output']] as const) {
      const portRefs: ReturnType<typeof ref>[] = []
      for (const port of block[key] ?? []) {
        const portId = `${id}.${port.name}`
        const item: RecordNode = { '@id': portId, '@type': `S231:${port.type}${direction}`, 'S231:label': port.name }
        if (port.description) item['S231:description'] = port.description
        if (port.brick_class) item['ace:brickClass'] = port.brick_class
        if (port.label) item['ace:pointLabel'] = port.label
        if (port.unit) item['ace:unit'] = port.unit
        if (port.cxf_unit) item['qudt:hasUnit'] = ref(port.cxf_unit)
        else if (port.unit && unitId(port.unit)) item['qudt:hasUnit'] = ref(unitId(port.unit))
        if (port.cxf_display_unit) item['S231:hasDisplayUnit'] = ref(port.cxf_display_unit)
        nodes.push(item); portRefs.push(ref(portId)); instances.push(ref(portId))
      }
      if (portRefs.length) node[`S231:has${direction}`] = portRefs
    }
    const params: ReturnType<typeof ref>[] = []
    for (const parameter of block.parameters ?? []) {
      const paramId = `${id}.${parameter.name}`
      const value = parameter.value
      const type = parameter.type ?? (typeof value === 'boolean' ? 'Boolean' : typeof value === 'string' ? 'String' : 'Real')
      const item: RecordNode = { '@id': paramId, '@type': 'S231:Parameter', 'S231:label': parameter.name, 'S231:isOfDataType': ref(`S231:${type}`), 'S231:value': value }
      if (parameter.description) item['S231:description'] = parameter.description
      if (parameter.cxf_unit) item['qudt:hasUnit'] = ref(parameter.cxf_unit)
      else if (parameter.unit && unitId(parameter.unit)) item['qudt:hasUnit'] = ref(unitId(parameter.unit))
      if (parameter.cxf_display_unit) item['S231:hasDisplayUnit'] = ref(parameter.cxf_display_unit)
      nodes.push(item); params.push(ref(paramId)); instances.push(ref(paramId))
    }
    if (params.length) node['S231:hasParameter'] = params
    if (instances.length) node['S231:hasInstance'] = instances
  }
  const visit = (block: Block, id: string, top = false) => {
    const standard = knownClass(block.block_type)
    const custom = `${BASE}type.${encodeURIComponent(id.slice(rootId.length) || block.name)}`
    const node: RecordNode = { '@id': id, '@type': top ? 'S231:Block' : standard ?? custom, 'S231:label': block.name }
    node['ace:blockType'] = block.block_type
    if (block.description) node['S231:description'] = block.description
    if (block.notes) node['ace:notes'] = block.notes
    if (block.blocks?.length) node['S231:containsBlock'] = block.blocks.map(child => ref(`${id}.${child.name}`))
    nodes.push(node)
    addPorts(block, id, node)
    if (!top && !standard) {
      const definition: RecordNode = { '@id': custom, '@type': block.blocks?.length ? 'S231:CompositeBlock' : 'S231:ElementaryBlock', 'S231:label': block.block_type }
      for (const key of ['S231:hasInput', 'S231:hasOutput', 'S231:hasParameter']) {
        const classPorts = refs(node[key]).map(instanceId => {
          const classPortId = `${custom}.${lastName(instanceId)}`
          const instance = nodes.find(item => item['@id'] === instanceId)!
          nodes.push({ ...instance, '@id': classPortId })
          return ref(classPortId)
        })
        if (classPorts.length) definition[key] = classPorts
      }
      nodes.push(definition)
    }
    for (const child of block.blocks ?? []) visit(child, `${id}.${child.name}`)
    for (const connection of block.connections ?? []) {
      const [from, to] = edgeEndpoints(connection)
      const source = nodes.find(item => item['@id'] === `${id}.${from}`)
      if (!source) throw new Error(`CXF export cannot resolve source ${id}.${from}`)
      const target = `${id}.${to}`
      if (!nodes.some(item => item['@id'] === target)) throw new Error(`CXF export cannot resolve target ${target}`)
      const links = asList(source['S231:isConnectedTo'])
      if (!links.some(item => item['@id'] === target)) source['S231:isConnectedTo'] = [...links, ref(target)]
    }
  }
  // Connections are emitted after all nested connector records exist.
  visit(graph, rootId, true)
  return { '@context': CONTEXT, '@graph': nodes }
}
