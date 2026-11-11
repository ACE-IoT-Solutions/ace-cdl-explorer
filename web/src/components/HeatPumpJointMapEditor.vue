<template>
  <details class="field joint-map-editor">
    <summary>Joint heat-pump performance map</summary>
    <p class="muted small">Optional physical equipment table over outdoor, hot-water, chilled-water, and load conditions. At full load it sets available heating and cooling capacity; at actual part load it sets each COP. CDL still selects mode and dispatch. Leave empty to use the separate teaching curves below.</p>
    <p class="muted small">Axes are increasing arrays; <code>load_fraction</code> must end at 1. Values are flattened in outdoor → hot water → chilled water → load order. Capacity fractions multiply nameplate kW. Add <code>simultaneous_cop</code> to every row to use combined useful heating plus cooling divided by compressor electricity during simultaneous service. Otherwise the teaching savings fraction applies.</p>
    <textarea v-model="draft" aria-label="Joint heat-pump performance map JSON" spellcheck="false" :disabled="disabled" placeholder="Paste a performance map as JSON…"/>
    <p v-if="error" class="error-text" role="alert">{{ error }}</p>
    <div class="joint-map-actions"><button class="outline-button" type="button" :disabled="disabled" @click="loadTemplate">Insert teaching template</button><button class="outline-button" type="button" :disabled="disabled" @click="clear">Use separate curves</button><button class="solid-button" type="button" :disabled="disabled" @click="apply">Save map</button></div>
  </details>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import type { HeatPumpJointMap } from '../lib/types'

const props = defineProps<{ value: HeatPumpJointMap | null; disabled: boolean }>()
const emit = defineEmits<{ save: [value: HeatPumpJointMap | null] }>()
const draft = ref(props.value ? JSON.stringify(props.value, null, 2) : '')
const error = ref('')
watch(() => props.value, value => { draft.value = value ? JSON.stringify(value, null, 2) : ''; error.value = '' })

function apply() {
  if (!draft.value.trim()) { emit('save', null); error.value = ''; return }
  try {
    const value = JSON.parse(draft.value) as HeatPumpJointMap
    const axes = [value.outdoor_c, value.hot_water_c, value.chilled_water_c, value.load_fraction]
    const limits = [[-60, 70], [-20, 100], [-20, 100], [0, 1]]
    if (axes.some((axis, dimension) => !Array.isArray(axis) || !axis.length || axis.length > 16 ||
      axis.some((entry, index) => !Number.isFinite(entry) || entry < limits[dimension][0] || entry > limits[dimension][1] || (index > 0 && entry <= axis[index - 1])))) throw new Error('Axes must contain increasing finite values within the listed temperature and load ranges (1–16 per axis).')
    if (value.load_fraction.at(-1) !== 1) throw new Error('Load fractions must end at 1.')
    const expected = axes.reduce((size, axis) => size * axis.length, 1)
    if (!Array.isArray(value.values) || value.values.length !== expected || expected > 4096) throw new Error(`Expected ${expected} value rows (maximum 4096).`)
    if (value.values.some(point => !point ||
      !Number.isFinite(point.heating_capacity_fraction) || point.heating_capacity_fraction < 0 || point.heating_capacity_fraction > 2 ||
      !Number.isFinite(point.cooling_capacity_fraction) || point.cooling_capacity_fraction < 0 || point.cooling_capacity_fraction > 2 ||
      !Number.isFinite(point.heating_cop) || point.heating_cop < 0.5 || point.heating_cop > 15 ||
      !Number.isFinite(point.cooling_cop) || point.cooling_cop < 0.5 || point.cooling_cop > 15 ||
      (point.simultaneous_cop !== undefined && (!Number.isFinite(point.simultaneous_cop) || point.simultaneous_cop < 0.5 || point.simultaneous_cop > 30)))) throw new Error('Each row needs capacity fractions 0–2, heating/cooling COP values 0.5–15, and optional simultaneous COP 0.5–30.')
    if (value.values.some(point => (point.simultaneous_cop !== undefined) !== (value.values[0].simultaneous_cop !== undefined))) throw new Error('Provide simultaneous_cop in every row or omit it from every row.')
    emit('save', value)
    error.value = ''
  } catch (caught) { error.value = caught instanceof Error ? caught.message : String(caught) }
}

function clear() { draft.value = ''; error.value = ''; emit('save', null) }
function loadTemplate() {
  draft.value = JSON.stringify({
    outdoor_c: [10], hot_water_c: [45], chilled_water_c: [7], load_fraction: [0, 1],
    values: [0, 1].map(() => ({ heating_capacity_fraction: 1, cooling_capacity_fraction: 1, heating_cop: 3.2, cooling_cop: 3.8, simultaneous_cop: 5.0 })),
  }, null, 2)
  error.value = ''
}
</script>

<style scoped>
.joint-map-editor textarea{width:100%;min-height:190px;margin-top:.5rem;padding:.6rem;border:1px solid var(--ace-border-strong);border-radius:5px;background:var(--ace-bg-panel);color:var(--ace-text);font:12px/1.4 monospace;resize:vertical}
.joint-map-actions{display:flex;flex-wrap:wrap;gap:.4rem;margin-top:.5rem}
.joint-map-actions button{min-height:32px;padding:.35rem .6rem;font-size:.72rem}
.error-text{color:#ffa682;font-size:.75rem}
</style>
