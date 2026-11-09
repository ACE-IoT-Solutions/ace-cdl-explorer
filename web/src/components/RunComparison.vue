<template>
  <section class="chart-card run-comparison">
    <div class="card-heading"><div><span class="eyebrow">CONTROL COMPARISON</span><h2>Runs in this browser</h2></div><span class="muted small">2025 WEATHER · TEN ZONES</span></div>
    <p class="muted small">Each value below comes from a completed simulation in this browser. Run both control programs to compare them. Edited controls, assumptions, and timesteps can change the result.</p>
    <p v-if="!runs.length" class="muted">No completed runs yet. Run the building simulation to start a comparison.</p>
    <template v-else>
      <div class="run-legend"><div v-for="run in runs" :key="run.id" class="run-legend-entry">
        <span class="run-swatch" :class="run.label.includes('G36') ? 'g36' : 'conventional'"/>
        <span><strong>{{ run.label }}</strong><small>{{ formatDate(run.completedAt) }} · {{ formatDuration(run.durationMs) }} run · {{ run.effectiveStepSeconds }} s steps<span v-if="run.requestedStepSeconds!==run.effectiveStepSeconds"> ({{ run.requestedStepSeconds }} s selected)</span></small></span>
      </div></div>
      <div class="run-metric-grid"><div v-for="metric in metrics" :key="metric.key" class="run-metric"><h3>{{ metric.label }} <small>{{ metric.unit }}</small></h3>
        <div v-for="run in runs" :key="run.id" class="run-bar-row"><span class="run-bar-name">{{ run.label }} · {{ formatShortDate(run.completedAt) }}</span><div class="run-bar-track"><div :class="run.label.includes('G36') ? 'g36' : 'conventional'" :style="{ width: `${barWidth(run.summary[metric.key], metric.key)}%` }"/></div><strong>{{ formatValue(run.summary[metric.key]) }}</strong></div>
      </div></div>
      <p class="muted small">Recent {{ runs.length }} completed {{ runs.length===1 ? 'run' : 'runs' }} are saved only in this browser. Longer bars mean larger values; lower present-zone comfort exceedance means fewer hours outside the occupied band.</p>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { Summary } from '../lib/types'
import type { SimulationRun } from '../lib/runHistory'

const props = defineProps<{ runs: SimulationRun[] }>()
const metrics: {key: keyof Summary; label: string; unit: string}[] = [
  {key:'electricity_kwh',label:'Electricity',unit:'kWh'},
  {key:'gas_kwh',label:'Boiler fuel',unit:'kWh'},
  {key:'occupied_unmet_zone_hours',label:'Present-zone comfort exceedance',unit:'zone-hours'},
]
const formatValue=(value:number)=>new Intl.NumberFormat(undefined,{maximumFractionDigits:value<10?2:0}).format(value)
const formatDate=(value:string)=>new Date(value).toLocaleString(undefined,{dateStyle:'medium',timeStyle:'short'})
const formatShortDate=(value:string)=>new Date(value).toLocaleString(undefined,{month:'short',day:'numeric',hour:'numeric',minute:'2-digit'})
const formatDuration=(value:number)=>value>=60_000?`${(value/60_000).toFixed(1)} min`:`${(value/1000).toFixed(1)} s`
function barWidth(value:number,key:keyof Summary){
  const max=Math.max(0,...props.runs.map(run=>run.summary[key]))
  return max>0 ? Math.max(0,Math.min(100,value/max*100)) : 0
}
</script>
