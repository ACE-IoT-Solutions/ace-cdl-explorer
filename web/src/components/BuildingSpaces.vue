<template>
  <section class="space-map" aria-label="Building spaces and HVAC zones">
    <div class="space-map-heading"><div><span class="eyebrow">BUILDING MODEL / BRICK + REC</span><h2>Spaces served by each control zone</h2><p>Each synthetic zone represents one room. The AHU feeds its VAV, the VAV feeds the HVAC zone, and the zone contains the room.</p></div><span class="space-count">{{ groups.reduce((total, group) => total + group.rooms.length, 0) }} spaces</span></div>
    <div class="space-groups"><div v-for="group in groups" :key="group.ahu.id" class="space-group"><h3>{{ group.ahu.label }} <small>brick:AHU</small></h3><div class="space-cards"><div v-for="item in group.rooms" :key="item.room.id" class="space-card"><strong>{{ item.room.label }}</strong><span>rec:Room · {{ item.zone.label }}</span><small>{{ item.vav.label }} feeds {{ item.zone.id }} · controller {{ item.zone.controller }}</small></div></div></div></div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { BrickEntity, BrickModel } from '../lib/types'

const props = defineProps<{ model: BrickModel }>()
const groups = computed(() => {
  const entities = new Map(props.model.entities.map(entity => [entity.id, entity]))
  const targets = (subject: string, predicate: string) => props.model.relationships
    .filter(relation => relation.subject === subject && relation.predicate === predicate)
    .map(relation => entities.get(relation.object)).filter((entity): entity is BrickEntity => !!entity)
  return props.model.entities.filter(entity => entity.class === 'brick:AHU').map(ahu => ({
    ahu,
    rooms: targets(ahu.id, 'brick:feeds').flatMap(vav =>
      targets(vav.id, 'brick:feeds').flatMap(zone =>
        targets(zone.id, 'brick:hasPart').filter(room => room.class === 'rec:Room')
          .map(room => ({ vav, zone, room })))),
  }))
})
</script>

<style scoped>
.space-map{margin:18px 0 22px;padding:22px;border:1px solid #3b4559;border-radius:12px;background:#202632;color:#f5f7fc}
.space-map-heading{display:flex;justify-content:space-between;gap:18px;align-items:start;margin-bottom:20px}
.space-map h2{font-size:1.3rem;margin:5px 0 6px}.space-map p{color:#b4c0cd;margin:0;max-width:760px;font-size:.88rem}
.space-count{white-space:nowrap;color:#d5e64a;font-weight:700;font-size:.8rem;text-transform:uppercase;letter-spacing:.08em}
.space-groups{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:16px}
.space-group{border:1px solid #3c4659;border-radius:9px;padding:16px;background:#191e29}
.space-group h3{font-size:1rem;margin:0 0 12px}.space-group h3 small{color:#9ce5ea;font-size:.7rem;margin-left:8px;font-weight:500}
.space-cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(185px,1fr));gap:8px}
.space-card{display:flex;flex-direction:column;gap:5px;padding:12px;border-radius:7px;background:#293241;border-left:3px solid #9ce5ea}
.space-card strong{font-size:.86rem}.space-card span{font-size:.76rem;color:#b9e8e9}.space-card small{font-size:.69rem;color:#b4c0cd;line-height:1.35}
@media(max-width:900px){.space-groups{grid-template-columns:1fr}.space-map-heading{flex-direction:column}}
</style>
