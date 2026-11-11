<template>
  <details class="field">
    <summary>Outdoor air quality · raw inputs</summary>
    <p class="muted small">The operator position and available readings enter the editable G36 pollution-mode CDL blocks. Empty readings mean that sensor is unavailable. CDL decides the mode, alarm, and economizer lockout.</p>
    <label class="field"><span>Operator switch</span><select v-model.number="scenario.pollution_operator" :disabled="running" @change="emit('change')"><option :value="0">Off</option><option :value="1">On · timed in CDL</option><option :value="2">Auto · available sensors</option></select></label>
    <label class="field"><span>On duration · seconds (15–604,800)</span><input v-model.number="scenario.pollution_on_duration_s" type="number" min="15" max="604800" step="15" :disabled="running" @change="emit('change')"/></label>
    <label class="field"><span>Local clock · UTC offset in hours</span><input v-model.number="scenario.local_utc_offset_hours" type="number" min="-12" max="14" step="0.25" :disabled="running" @change="emit('change')"/></label>
    <div class="field"><span>Clock offset changes</span><p class="muted small">Add UTC-hour transition times for daylight saving or other local clock changes. The graph's reminder time is an editable CDL constant, in seconds after local midnight.</p><div v-for="(event,index) in scenario.clock_offset_events" :key="index" class="performance-point"><label><span>Start · UTC hour of year</span><input v-model.number="event.start_hour" type="number" min="0" max="8760" step="0.25" :disabled="running" @change="emit('change')"/></label><label><span>New UTC offset · hours</span><input v-model.number="event.utc_offset_hours" type="number" min="-12" max="14" step="0.25" :disabled="running" @change="emit('change')"/></label><button class="outline-button" type="button" :disabled="running" @click="scenario.clock_offset_events.splice(index,1);emit('change')">Remove clock change</button></div><button class="outline-button" type="button" :disabled="running" @click="scenario.clock_offset_events.push({start_hour:0,utc_offset_hours:scenario.local_utc_offset_hours});emit('change')">Add clock change</button></div>
    <label v-for="reading in readings" :key="reading.key" class="field"><span>{{ reading.label }}</span><input type="number" :min="0" :max="reading.max" :step="reading.step" :value="scenario[reading.key] ?? ''" :disabled="running" placeholder="Sensor unavailable" @change="setReading(reading.key, $event)"/></label>
    <div class="field"><span>Timed reading changes</span><p class="muted small">Each event temporarily replaces one raw reading. Leave value blank to make that sensor unavailable for the interval. The CDL timers decide when Auto changes state.</p><div v-for="(event,index) in scenario.air_quality_events" :key="index" class="performance-point"><label><span>Pollutant</span><select v-model="event.pollutant" :disabled="running" @change="emit('change')"><option value="pm25">PM2.5</option><option value="ozone">Ozone</option><option value="aqi">AQI</option></select></label><label><span>Reading</span><input type="number" min="0" step="any" :value="event.value ?? ''" :disabled="running" placeholder="Unavailable" @change="setEventValue(index,$event)"/></label><label><span>Start · hour of year</span><input v-model.number="event.start_hour" type="number" min="0" max="8760" step="0.25" :disabled="running" @change="emit('change')"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" max="8760" step="0.25" :disabled="running" @change="emit('change')"/></label><button class="outline-button" type="button" :disabled="running" @click="scenario.air_quality_events.splice(index,1);emit('change')">Remove reading change</button></div><button class="outline-button" type="button" :disabled="running" @click="addEvent">Add reading change</button></div>
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
function setEventValue(index: number, event: Event) {
  const value = (event.target as HTMLInputElement).value.trim()
  scenario.air_quality_events[index].value = value === '' ? null : Number(value)
  emit('change')
}
function addEvent() {
  scenario.air_quality_events.push({pollutant:'pm25',value:150,start_hour:24,duration_hours:2})
  emit('change')
}
</script>
