<template>
  <div ref="card" class="brick-point" :class="[direction==='input'?'brick-point-input':'brick-point-output',compact?'brick-point-compact':'']" tabindex="0" :aria-describedby="visible ? tooltipId : undefined" @pointerenter="showCard" @pointerleave="hideCard" @focus="showCard" @blur="hideCard">
    <template v-if="compact"><span class="brick-point-mark">◆</span><strong>{{ name }}</strong><span class="brick-point-class">brick:{{ brickClass }}</span></template>
    <template v-else><span class="brick-point-heading">BRICK POINT · {{ direction==='input' ? 'SIM INPUT' : 'SIM OUTPUT' }}</span><strong>{{ name }}</strong><span class="brick-point-class">brick:{{ brickClass }}</span><b v-if="value!==undefined">{{ value }}</b></template>
    <i class="brick-point-anchor" :data-anchor="anchorId"/>
  </div>
  <Teleport to="body"><div v-if="visible" ref="tooltip" :id="tooltipId" class="brick-point-tooltip" :style="{left:position.left+'px',top:position.top+'px',visibility:position.ready?'visible':'hidden'}" role="tooltip">
    <span class="brick-point-tooltip-kicker">BRICK POINT · SIM {{ direction==='input' ? 'INPUT' : 'OUTPUT' }}</span>
    <strong>{{ label || name }}</strong>
    <dl><div><dt>Connector</dt><dd>{{ name }}</dd></div><div><dt>Brick class</dt><dd class="brick-point-tooltip-class">brick:{{ brickClass }}</dd></div><div v-if="dataType"><dt>Data type</dt><dd>{{ dataType }}</dd></div><div><dt>Unit</dt><dd>{{ unit || 'Not declared' }}</dd></div><div v-if="value!==undefined"><dt>Live value</dt><dd>{{ value }}</dd></div></dl>
    <p v-if="description && description!==label">{{ description }}</p><p v-if="generic" class="brick-point-tooltip-note">Generic modeled or calculated point; this class does not imply a physical sensor.</p>
  </div></Teleport>
</template>

<script setup lang="ts">
import { nextTick, onUnmounted, ref, useId } from 'vue'

defineProps<{ direction:'input'|'output'; name:string; brickClass:string; unit?:string; value?:string; anchorId?:string; compact?:boolean; generic?:boolean; label?:string; description?:string; dataType?:string }>()
const tooltipId=`brick-point-${useId()}`
const card=ref<HTMLElement|null>(null)
const tooltip=ref<HTMLElement|null>(null)
const visible=ref(false)
const position=ref({left:0,top:0,ready:false})
function placeCard(){
  if(!card.value||!tooltip.value)return
  const anchor=card.value.getBoundingClientRect(),tip=tooltip.value.getBoundingClientRect()
  const left=Math.max(12,Math.min(anchor.left,window.innerWidth-tip.width-12))
  const below=anchor.bottom+10
  const above=anchor.top-tip.height-10
  const top=below+tip.height<=window.innerHeight-12?below:above>=12?above:Math.max(12,Math.min(below,window.innerHeight-tip.height-12))
  position.value={left,top,ready:true}
}
async function showCard(){visible.value=true;await nextTick();placeCard()}
function hideCard(){visible.value=false;position.value.ready=false}
onUnmounted(hideCard)
</script>
