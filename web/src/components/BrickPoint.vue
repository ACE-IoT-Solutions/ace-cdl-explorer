<template>
  <div class="brick-point" :class="[direction==='input'?'brick-point-input':'brick-point-output',compact?'brick-point-compact':'']" :title="`${name} · brick:${brickClass} · ${unit ?? 'unit not declared'}${generic ? ' · modeled or calculated point' : ''}`">
    <template v-if="compact"><span class="brick-point-mark">◆</span><strong>{{ name }}</strong><span class="brick-point-class">brick:{{ brickClass }}</span></template>
    <template v-else><span class="brick-point-heading">BRICK POINT · {{ direction==='input' ? 'SIM INPUT' : 'SIM OUTPUT' }}</span><strong>{{ name }}</strong><span class="brick-point-class">brick:{{ brickClass }}</span><b v-if="value!==undefined">{{ value }}</b></template>
    <i class="brick-point-anchor" :data-anchor="anchorId"/>
  </div>
</template>

<script setup lang="ts">
defineProps<{ direction:'input'|'output'; name:string; brickClass:string; unit?:string; value?:string; anchorId?:string; compact?:boolean; generic?:boolean }>()
</script>
