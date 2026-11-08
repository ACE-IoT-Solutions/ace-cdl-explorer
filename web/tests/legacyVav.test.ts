import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { expandLegacyVavLeaves, hasLegacyVavLeaf } from '../src/lib/legacyVav'
import type { Block } from '../src/lib/types'

const template = JSON.parse(readFileSync(new URL('../public/examples/vav_composite_template.json', import.meta.url), 'utf8')) as Block

test('python-cdl VAV shortcut expands into editable CDL blocks with its tuning', () => {
  const legacy: Block = {
    name: 'VAVController_zone_east', block_type: 'VAVBoxController',
    inputs: template.inputs, outputs: template.outputs,
    parameters: [
      { name: 'cooling_setpoint', value: 23.5 },
      { name: 'heating_setpoint', value: 20.5 },
      { name: 'min_airflow', value: 0.2 },
      { name: 'max_airflow', value: 0.95 },
      { name: 'min_damper_position', value: 0.15 },
      { name: 'kp_damper', value: 0.6 },
      { name: 'ki_damper', value: 0.12 },
      { name: 'kp_reheat', value: 0.35 },
      { name: 'ki_reheat', value: 0.06 },
    ],
  }
  assert.equal(hasLegacyVavLeaf(legacy), true)
  const expanded = expandLegacyVavLeaves(legacy, template)
  assert.equal(hasLegacyVavLeaf(expanded), false)
  assert.equal(expanded.name, legacy.name)
  assert.ok(expanded.blocks?.length)
  const parameter = (name: string, key: string) => expanded.blocks?.find(block => block.name === name)?.parameters?.find(item => item.name === key)?.value
  assert.equal(parameter('cool_setpoint', 'k'), 23.5)
  assert.equal(parameter('heat_setpoint', 'k'), 20.5)
  assert.equal(parameter('flow_span', 'k'), 0.75)
  assert.equal(parameter('cool_pi', 'Ti'), 4500)
  assert.equal(parameter('heat_pi', 'Ti'), 5250)
  assert.equal(legacy.blocks, undefined)
})
