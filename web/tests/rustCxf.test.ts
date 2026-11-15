import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFileSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import jsonld from 'jsonld'

const root = fileURLToPath(new URL('../../', import.meta.url))
const load = (path: string) => JSON.parse(readFileSync(resolve(root, path), 'utf8'))
function convert(input: string, output: string, legacy = false) {
  execFileSync('cargo', ['run', '--quiet', '--release', '-p', 'cdl-integration-tests', '--example', 'convert', '--',
    resolve(root, input), output, ...(legacy ? ['legacy'] : [])], { cwd: root, stdio: 'pipe' })
  return JSON.parse(readFileSync(output, 'utf8'))
}
function iri(value: unknown) {
  return typeof value === 'string' ? value.replace(/^unit:/, 'http://qudt.org/vocab/unit#') : value
}
function subset(actual: any, expected: any, path = '') {
  if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), path)
    assert.equal(actual.length, expected.length, path)
    const key = (v: any) => v?.name ?? (v?.from && v?.to ? `${v.from}>${v.to}` : JSON.stringify(v))
    const unordered = /\.(inputs|outputs|parameters|blocks|connections)$/.test(path)
    const a = unordered ? [...actual].sort((a, b) => key(a).localeCompare(key(b))) : actual
    const b = unordered ? [...expected].sort((a, b) => key(a).localeCompare(key(b))) : expected
    b.forEach((v, i) => subset(a[i], v, `${path}[${i}]`))
  } else if (expected && typeof expected === 'object') {
    for (const [key, value] of Object.entries(expected)) subset(actual?.[key], value, `${path}.${key}`)
  } else {
    assert.deepEqual(path.endsWith('.cxf_unit') || path.endsWith('.cxf_display_unit') ? iri(actual) : actual,
      path.endsWith('.cxf_unit') || path.endsWith('.cxf_display_unit') ? iri(expected) : expected, path)
  }
}

test('Rust CoolingCoil import preserves every legacy-recognized field from the frozen TS importer projection', () => {
  const directory = mkdtempSync(join(tmpdir(), 'rust-cxf-parity-'))
  try {
    const expected = load('web/tests/fixtures/CoolingCoil-legacy-project.json')
    const actual = convert('web/tests/fixtures/CoolingCoil.jsonld', join(directory, 'legacy.json'), true)
    // Rust additionally retains unknown namespaced metadata. Parameter ordering
    // and compact/expanded QUDT spelling do not change the graph's meaning.
    subset(actual, expected)
  } finally { rmSync(directory, { recursive: true, force: true }) }
})

test('Rust exports for pinned families and both buildings expand as JSON-LD with local structural references', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'rust-cxf-jsonld-'))
  try {
    const manifest = load('crates/cdl-integration-tests/tests/cxf/fixtures/manifest.json')
    assert.equal(manifest.revision, 'f75058d25bafb8c503e894dd193d3fa8037aef23')
    const inputs = manifest.fixtures.map((fixture: { file: string; sha256: string }) => {
      const path = `crates/cdl-integration-tests/tests/cxf/fixtures/${fixture.file}`
      assert.equal(createHash('sha256').update(readFileSync(resolve(root, path))).digest('hex'), fixture.sha256)
      return path
    }).concat(['web/public/examples/four_pipe_building.json', 'web/public/examples/four_pipe_g36.json'])
    for (const [index, input] of inputs.entries()) {
      const document = convert(input, join(directory, `${index}.json`))
      assert.equal(document['@context'].qudt, 'http://qudt.org/schema/qudt/')
      const expanded = await jsonld.expand(document)
      assert.ok(expanded.length > 0, input)
      const nodes = document['@graph'] as Record<string, any>[]
      const ids = new Set(nodes.map(node => node['@id']))
      assert.equal(ids.size, nodes.length, input)
      for (const node of nodes) {
        assert.ok(!(node['@type'] === 'S231:ElementaryBlock' && node['@id'].startsWith('http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.')), 'standard classes must be referenced')
        for (const key of ['S231:containsBlock', 'S231:hasInput', 'S231:hasOutput', 'S231:hasParameter', 'S231:hasConstant', 'S231:hasInstance', 'S231:isConnectedTo', 'ace:connections', 'ace:from', 'ace:to']) {
          for (const link of node[key] == null ? [] : Array.isArray(node[key]) ? node[key] : [node[key]]) {
            assert.ok(ids.has(link['@id']), `${input}: ${node['@id']} ${key} ${link['@id']}`)
          }
        }
      }
      assert.ok(ids.has(document['ace:rootBlock']['@id']))
    }
  } finally { rmSync(directory, { recursive: true, force: true }) }
})
