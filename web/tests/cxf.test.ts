import './wasm'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import jsonld from 'jsonld'
import { exportCxf, importCxf, type CxfDocument } from '../src/lib/cxf'
import { palette, type Block } from '../src/lib/types'

const read = <T>(path: string): T => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8')) as T
const core = read<CxfDocument>('../public/schemas/CXF-Core.jsonld')

test('imports an official elementary CXF block as an executable CDL type', () => {
  const graph = importCxf(read<CxfDocument>('./fixtures/Add.jsonld'))
  assert.equal(graph.block_type, 'Buildings.Controls.OBC.CDL.Reals.Add')
  assert.deepEqual(graph.inputs?.map(port => port.name), ['u1', 'u2'])
  assert.deepEqual(graph.outputs?.map(port => port.name), ['y'])
})

test('imports an official modelica-json CXF reference and its connector wiring', () => {
  const reference = read<CxfDocument>('./fixtures/CoolingCoil.jsonld')
  const graph = importCxf(reference)
  assert.equal(graph.name, 'CoolingCoil')
  assert.equal(graph.blocks?.length, 6)
  assert.ok(graph.inputs?.some(port => port.name === 'TAirSup' && port.type === 'Real'))
  assert.equal(graph.inputs?.find(port => port.name === 'TAirSup')?.cxf_unit, 'http://qudt.org/vocab/unit#K')
  assert.equal(graph.inputs?.find(port => port.name === 'TAirSup')?.cxf_display_unit, 'http://qudt.org/vocab/unit#DEG_C')
  assert.ok(graph.blocks?.some(block => block.name === 'and2' && block.inputs?.some(port => port.name === 'u2' && port.type === 'Boolean')))
  assert.ok(graph.connections?.some(edge => edge.from === 'u1SupFan' && edge.to === 'and2.u2'))
  assert.ok(graph.connections?.some(edge => edge.from === 'conCoi.y' && edge.to === 'switch.u1'))
  const roundtrip = exportCxf(graph)
  const port = roundtrip['@graph'].find(node => node['@id'].endsWith('.TAirSup'))
  assert.deepEqual(port?.['qudt:hasUnit'], { '@id': 'http://qudt.org/vocab/unit#K' })
  assert.deepEqual(port?.['S231:hasDisplayUnit'], { '@id': 'http://qudt.org/vocab/unit#DEG_C' })
})

test('accepts the connectedTo spelling in the CXF specification', () => {
  const reference = read<CxfDocument>('./fixtures/CoolingCoil.jsonld')
  for (const node of reference['@graph']) if (node['S231:isConnectedTo']) {
    node['S231:connectedTo'] = node['S231:isConnectedTo']
    delete node['S231:isConnectedTo']
  }
  const graph = importCxf(reference)
  assert.ok(graph.connections?.some(edge => edge.from === 'u1SupFan' && edge.to === 'and2.u2'))
})

test('exports a composite graph as CXF and preserves ports, parameters, notes and wires', () => {
  const graph: Block = {
    name: 'ExampleSeq', block_type: 'Composite', category: 'composite', notes: 'Sequence note',
    inputs: [{ name: 'u', type: 'Real', unit: '°C' }], outputs: [{ name: 'y', type: 'Real', unit: '°C' }],
    blocks: [{ name: 'gain', block_type: 'Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter', inputs: [{ name: 'u', type: 'Real' }], outputs: [{ name: 'y', type: 'Real' }], parameters: [{ name: 'k', value: 2 }] }],
    connections: [{ from: 'u', to: 'gain.u' }, { from: 'gain.y', to: 'y' }],
  }
  const cxf = exportCxf(graph)
  assert.equal(cxf['@context'].S231, 'http://data.ashrae.org/S231#')
  assert.ok(cxf['@graph'].some(node => node['@id'].endsWith('.gain') && node['@type'] === 'http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter'))
  const imported = importCxf(cxf)
  assert.equal(imported.notes, 'Sequence note')
  const wires = (block: Block) => block.connections?.map(({from,to})=>`${from}>${to}`).sort()
  assert.deepEqual(wires(imported), wires(graph))
  assert.equal(imported.blocks?.[0].parameters?.[0].value, 2)
  assert.equal(imported.blocks?.[0].block_type, graph.blocks?.[0].block_type)
  assert.equal(imported.inputs?.[0].unit, '°C')
})

test('PIDWithReset controller mode survives CXF export and import', () => {
  const controller = palette.find(block => block.type.endsWith('PIDWithReset'))
  assert.ok(controller)
  const graph: Block = {
    name: 'ControllerSequence', block_type: 'Composite', category: 'composite',
    inputs: [{name:'u_s',type:'Real'},{name:'u_m',type:'Real'},{name:'trigger',type:'Boolean'}],
    outputs: [{name:'y',type:'Real'}],
    blocks: [{name:'controller',block_type:controller.type,category:'elementary',
      inputs:controller.inputs,outputs:controller.outputs,
      parameters:controller.parameters.map(parameter => ({...parameter,value:parameter.name==='controllerType'?'PID':parameter.value}))}],
    connections: [{from:'u_s',to:'controller.u_s'},{from:'u_m',to:'controller.u_m'},
      {from:'trigger',to:'controller.trigger'},{from:'controller.y',to:'y'}],
  }
  const imported = importCxf(exportCxf(graph))
  assert.equal(imported.blocks?.[0].parameters?.find(parameter => parameter.name==='controllerType')?.value,'PID')
})

test('the bundled building exports with resolvable CXF connections', () => {
  const graph = read<Block>('../public/examples/four_pipe_building.json')
  const cxf = exportCxf(graph)
  const imported = importCxf(cxf)
  assert.equal(imported.blocks?.length, graph.blocks?.length)
  assert.equal(imported.connections?.length, graph.connections?.length)
  assert.equal(imported.blocks?.[0].block_type, graph.blocks?.[0].block_type)
  assert.ok(cxf['@graph'].every(node => typeof node['@id'] === 'string'))
})

test('the occupancy-aware building preserves its sensor points and wiring in CXF', () => {
  const graph = read<Block>('../public/examples/four_pipe_g36.json')
  const imported = importCxf(exportCxf(graph))
  assert.equal(imported.connections?.length, graph.connections?.length)
  assert.equal(imported.inputs?.find(port=>port.name==='zone_01_occupied')?.brick_class,'Occupancy_Sensor')
  const zone=imported.blocks?.find(block=>block.name==='zone_01')
  assert.ok(zone?.connections?.some(edge=>edge.from==='zone_presence_raw'&&edge.to==='presence_delay.u'))
  assert.ok(zone?.connections?.some(edge=>edge.from==='occupancy_latch.y'&&edge.to==='occupancy_confirmed'))
})

test('bundled CXF expands as JSON-LD with resolved connector references', async () => {
  const graph = read<Block>('../public/examples/four_pipe_building.json')
  const cxf = exportCxf(graph)
  const expanded = await jsonld.expand(cxf)
  assert.ok(expanded.length > 0)
  const ids = new Set(cxf['@graph'].map(node => node['@id']))
  for (const node of cxf['@graph']) for (const key of ['S231:containsBlock', 'S231:hasInput', 'S231:hasOutput', 'S231:hasParameter', 'S231:hasInstance', 'S231:isConnectedTo']) {
    const links = node[key]
    for (const link of links == null ? [] : Array.isArray(links) ? links : [links]) assert.ok(ids.has((link as { '@id': string })['@id']), `${node['@id']} ${key} has an unresolved target`)
  }
})

test('bundled blocks using official CDL classes have matching connector interfaces', () => {
  const library = new Map(core['@graph'].map(node => [node['@id'], node]))
  let checked = 0
  const portNames = (value: unknown) => (value == null ? [] : Array.isArray(value) ? value : [value]).map((ref: { '@id': string }) => ref['@id'].split('.').at(-1)).sort()
  const visit = (block: Block) => {
    const type = block.block_type.startsWith('CDL.') ? `Buildings.Controls.OBC.${block.block_type}` : block.block_type
    const definition = library.get(`http://data.ashrae.org/S231#${type}`)
    if (definition) {
      checked++
      assert.deepEqual((block.inputs ?? []).map(port => port.name).sort(), portNames(definition['S231:hasInput']), `${block.name} inputs differ from CXF-Core`)
      assert.deepEqual((block.outputs ?? []).map(port => port.name).sort(), portNames(definition['S231:hasOutput']), `${block.name} outputs differ from CXF-Core`)
      const parameters = new Set(portNames(definition['S231:hasParameter']))
      for (const parameter of block.parameters ?? []) assert.ok(parameters.has(parameter.name), `${block.name}.${parameter.name} is not a defined parameter`)
    }
    for (const child of block.blocks ?? []) visit(child)
  }
  for (const entry of palette) visit({ name: entry.label, block_type: entry.type, inputs: entry.inputs, outputs: entry.outputs, parameters: entry.parameters })
  assert.ok(checked > 0, 'No official CDL class interfaces were checked')
})

test('imports controller enumeration declarations without disguising them as Real', () => {
  const graph=importCxf(read<CxfDocument>('./fixtures/CoolingCoil.jsonld'))
  const mode=graph.blocks?.find(block=>block.name==='conCoi')?.parameters?.find(p=>p.name==='controllerType')
  assert.equal(mode?.type,'http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Types.SimpleController')
})


test('legacy class aliases retain their original spelling through the wasm codec', () => {
  const graph: Block = {name:'gain',block_type:'CDL.Reals.MultiplyByParameter',
    inputs:[{name:'u',type:'Real'}],outputs:[{name:'y',type:'Real'}],parameters:[{name:'k',value:2}]}
  const imported = importCxf(exportCxf(graph))
  assert.equal(imported.block_type, graph.block_type)
  assert.equal(imported.parameters?.find(p=>p.name==='k')?.value, 2)
})


test('the wasm format boundary reports readable import errors', () => {
  assert.throws(() => importCxf(JSON.parse('{"@context":"https://example.org/remote","@graph":[]}')), /inline prefix contexts/)
})
