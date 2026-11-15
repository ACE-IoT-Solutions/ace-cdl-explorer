import { cxf_export, cxf_import } from '../wasm/ace_cdl_wasm.js'
import type { Block } from './types'
import type { Diagnostic } from './generated'

interface RecordNode extends Record<string, unknown> { '@id': string; '@type'?: string }
export interface CxfDocument {
  '@context': Record<string, string>
  '@graph': RecordNode[]
}
// The application initializes wasm before mounting; Node tests initialize the same
// generated module explicitly. Format semantics and the pinned core live in Rust.
export function importCxf(document: CxfDocument): Block {
  return converted<Block>(() => cxf_import(JSON.stringify(document)))
}
export function exportCxf(graph: Block): CxfDocument {
  return converted<CxfDocument>(() => cxf_export(JSON.stringify(graph)))
}

function converted<T>(call: () => string): T {
  try { return JSON.parse(call()) as T }
  catch (error) {
    let message = String(error)
    try {
      const diagnostics = JSON.parse(message) as Diagnostic[]
      if (Array.isArray(diagnostics)) message = diagnostics.map(d => `${d.path ? `${d.path}: ` : ''}${d.message}`).join('; ')
    } catch { /* Other runtime errors already have a readable message. */ }
    throw new Error(message)
  }
}
