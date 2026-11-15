import { readFileSync } from 'node:fs'
import { initSync } from '../src/wasm/ace_cdl_wasm.js'
initSync({ module: readFileSync(new URL('../src/wasm/ace_cdl_wasm_bg.wasm', import.meta.url)) })
