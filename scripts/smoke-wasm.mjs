import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import init, { execute_json, validate_json } from '../web/src/wasm/ace_cdl_runtime.js'

await init({ module_or_path: readFileSync(new URL('../web/src/wasm/ace_cdl_runtime_bg.wasm', import.meta.url)) })

for (const [file, expected] of [
  ['obc_zone_status.json', 'yEndSetUp'],
  ['vav_composite_template.json', 'damper_position'],
]) {
  const graph = JSON.parse(readFileSync(new URL(`../web/public/examples/${file}`, import.meta.url)))
  assert.deepEqual(JSON.parse(validate_json(JSON.stringify(graph))), [], `${file} graph validation`)
  const inputs = Object.fromEntries(graph.inputs.map(port => [port.name,
    port.type === 'Boolean' ? false : port.unit === '°C' ? 22 : 1]))
  const result = JSON.parse(execute_json(JSON.stringify(graph), JSON.stringify(inputs), 900))
  assert.ok(Object.hasOwn(result, expected), `${file} missing ${expected}`)
  console.log(`WASM quick test passed: ${file}`)
}
