<template>
  <svg class="block-symbol" viewBox="-116 -116 232 232" fill="none" stroke="currentColor" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" :data-symbol-source="symbol.source" :data-symbol-kind="symbol.source ? 'buildings' : 'ace'">
    <g v-if="symbol.source" v-html="symbol.graphics"/>
    <g v-else-if="composite" class="block-symbol-composite">
      <rect x="-96" y="-96" width="192" height="192" rx="12"/>
      <rect x="-65" y="-32" width="46" height="64" rx="5"/>
      <rect x="22" y="-32" width="46" height="64" rx="5"/>
      <path d="M-96 0h31m46 0h41m46 0h28"/>
    </g>
    <g v-else>
      <rect x="-96" y="-96" width="192" height="192" rx="12"/>
      <text x="0" y="2" text-anchor="middle" dominant-baseline="central" stroke="none" fill="currentColor" :font-size="type==='Filter' ? 36 : 80">{{ type==='Filter' ? '1/(Ts+1)' : 'ƒ' }}</text>
    </g>
  </svg>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { symbolForType } from '../lib/blockSymbols'
const props = defineProps<{ type: string; composite?: boolean }>()
const symbol = computed(() => symbolForType(props.type))
</script>

<style>
.block-symbol {
  --block-symbol-surface: var(--ace-bg-panel);
  --block-symbol-muted: var(--ace-text-subtle);
  display: block;
  width: 36px;
  height: 36px;
  flex: none;
  color: var(--ace-secondary);
  font-family: var(--ace-font-heading);
  font-weight: 600;
}
.palette-item .block-symbol { width: 30px; height: 30px; color: var(--ace-primary); }
.graph-node .node-head { padding-left: 3.65rem; }
.node-head > .block-symbol { position: absolute; top: .45rem; left: .55rem; --block-symbol-surface: #2e3445; }
.node-head > span:first-of-type { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.block-glossary-heading > .block-symbol { width: 52px; height: 52px; margin-bottom: .65rem; }
</style>
