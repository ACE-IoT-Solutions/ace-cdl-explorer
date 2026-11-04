import { edgeEndpoints, type Block } from './types'

export interface GraphSignal { id: string; label: string; kind: 'input' | 'output' | 'connection'; type: string }

export function graphSignals(graph: Block): GraphSignal[] {
  const signals: GraphSignal[] = []
  function visit(container: Block, path: string) {
    for (const port of container.inputs ?? []) {
      signals.push({ id: `${path}|${port.name}`, label: `${path}.${port.name}`, kind: 'input', type: port.type })
    }
    for (const port of container.outputs ?? []) {
      signals.push({ id: `${path}|output.${port.name}`, label: `${path}.${port.name}`, kind: 'output', type: port.type })
    }
    for (const child of container.blocks ?? []) {
      for (const port of child.outputs ?? []) {
        signals.push({ id: `${path}|${child.name}.${port.name}`, label: `${path}.${child.name}.${port.name}`, kind: 'output', type: port.type })
      }
    }
    for (const [index, edge] of (container.connections ?? []).entries()) {
      const [source, target] = edgeEndpoints(edge)
      const targetName = target.includes('.') ? target : `${container.name}.${target}`
      signals.push({ id: `${path}|edge.${index}`, label: `${path}: ${source} → ${targetName}`, kind: 'connection', type: 'signal' })
    }
    for (const child of container.blocks ?? []) {
      if (child.blocks?.length) visit(child, `${path}.${child.name}`)
    }
  }
  visit(graph, graph.name)
  return signals
}
