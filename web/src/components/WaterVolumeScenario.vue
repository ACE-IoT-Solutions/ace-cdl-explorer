<template>
  <details class="field">
    <summary>Water-loop physical storage</summary>
    <p class="muted small">Each mixed loop volume stores heat or cooling between source production and coil delivery. CDL still commands the pumps, setpoints, and equipment.</p>
    <div class="terminal-flow-grid">
      <label v-for="(_, index) in scenario.water_volume_kg" :key="index">
        <span>{{ index === 0 ? 'Hot' : 'Chilled' }} loop water mass · kg</span>
        <input v-model.number="scenario.water_volume_kg[index]" type="number" min="100" max="100000" step="100" :disabled="running" @change="emit('change')" />
      </label>
    </div>
  </details>
</template>

<script setup lang="ts">
import type { Scenario } from '../lib/types'

defineProps<{ scenario: Scenario; running: boolean }>()
const emit = defineEmits<{ change: [] }>()
</script>
