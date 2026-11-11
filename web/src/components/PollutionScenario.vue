<template>
  <details class="field">
    <summary>Outdoor air quality · raw inputs</summary>
    <p class="muted small">The operator position and available readings enter the editable G36 pollution-mode CDL blocks. Empty readings mean that sensor is unavailable. CDL decides the mode, alarm, and economizer lockout.</p>
    <label class="field"><span>Operator switch</span><select v-model.number="scenario.pollution_operator" :disabled="running" @change="emit('change')"><option :value="0">Off</option><option :value="1">On · timed in CDL</option><option :value="2">Auto · available sensors</option></select></label>
    <label v-for="reading in readings" :key="reading.key" class="field"><span>{{ reading.label }}</span><input type="number" :min="0" :max="reading.max" :step="reading.step" :value="scenario[reading.key] ?? ''" :disabled="running" placeholder="Sensor unavailable" @change="setReading(reading.key, $event)"/></label>
    <p class="muted small">The example PM2.5 limit in CDL assumes MERV 13 filtration. Set the limits for the actual AHU before interpreting results.</p>
  </details>
</template>

<script setup lang="ts">
import type { Scenario } from '../lib/types'

const { scenario, running } = defineProps<{ scenario: Scenario; running: boolean }>()
const emit = defineEmits<{ change: [] }>()
type Reading = 'outdoor_pm25_ug_m3' | 'outdoor_ozone_ppm' | 'outdoor_aqi'
const readings: {key: Reading; label: string; max: number; step: number}[] = [
  {key: 'outdoor_pm25_ug_m3', label: 'Outdoor PM2.5 · µg/m³', max: 2000, step: 1},
  {key: 'outdoor_ozone_ppm', label: 'Outdoor ozone · ppm', max: 10, step: 0.01},
  {key: 'outdoor_aqi', label: 'Outdoor AQI', max: 500, step: 1},
]
function setReading(key: Reading, event: Event) {
  const value = (event.target as HTMLInputElement).value.trim()
  scenario[key] = value === '' ? null : Number(value)
  emit('change')
}
</script>
