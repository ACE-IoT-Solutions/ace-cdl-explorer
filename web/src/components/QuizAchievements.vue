<template>
  <section class="achievements" aria-labelledby="quiz-share-title">
    <h3 id="quiz-share-title">Share your achievement</h3>
    <p>Copy the post text and a score-bearing PNG. Your email is never included.</p>
    <p class="share-status" role="status">{{ status }}</p>
    <div class="badges">
      <article v-for="kind in kinds" :key="kind" class="badge-card" :class="{locked:kind==='mastery' && !mastery}">
        <div class="badge" :class="kind">
          <span class="badge-icon" aria-hidden="true">{{ kind==='mastery' ? '★' : '◆' }}</span>
          <div><small>ACE CDL EXPLORER</small><strong>{{ kind==='quiz' ? `${percentage}% · Quiz complete` : '100% · Question bank mastered' }}</strong><span>{{ kind==='quiz' ? `${score} of ${total} correct` : `All ${bankTotal} questions answered correctly` }}</span></div>
        </div>
        <div v-if="kind==='quiz' || mastery" class="badge-actions">
          <button class="outline-button" :data-testid="`copy-${kind}-badge`" @click="copyText(kind)">Copy post text</button>
          <button class="outline-button" :data-testid="`copy-${kind}-image`" @click="copyImage(kind)">Copy badge image</button>
          <button class="outline-button" :data-testid="`download-${kind}-image`" @click="download(kind)">Download PNG</button>
        </div>
        <p v-else>Unlock at {{ bankTotal }} / {{ bankTotal }} answered correctly.</p>
      </article>
    </div>
  </section>
</template>
<script setup lang="ts">
import { computed, ref } from 'vue'
import { badgeImageBlob, badgeText, copyPostText, downloadBadge, quizShareUrl, type BadgeKind, type QuizAchievement } from '../lib/quiz-social'
const props = defineProps<QuizAchievement>()
const kinds: BadgeKind[] = ['quiz', 'mastery']
const status = ref('')
const percentage = computed(() => Math.round(props.score / props.total * 100))
const mastery = computed(() => props.masteredTotal >= props.bankTotal)
async function copyText(kind: BadgeKind) {
  try { await copyPostText(badgeText(kind, props, quizShareUrl(import.meta.env.BASE_URL, location.href))); status.value = 'Post text copied to your clipboard.' }
  catch { status.value = 'Could not copy text. Check your browser clipboard permissions.' }
}
async function download(kind: BadgeKind) {
  try { downloadBadge(await badgeImageBlob(kind, props), kind, props); status.value = 'Badge PNG downloaded.' }
  catch { status.value = 'Could not create the badge image in this browser.' }
}
async function copyImage(kind: BadgeKind) {
  try {
    if (navigator.clipboard?.write && typeof ClipboardItem !== 'undefined') {
      // Pass the promise immediately to preserve user activation in Safari.
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': badgeImageBlob(kind, props) })])
      status.value = 'Badge image copied. Paste it into your social post.'
    } else {
      downloadBadge(await badgeImageBlob(kind, props), kind, props)
      status.value = 'Clipboard images are unavailable, so the PNG was downloaded.'
    }
  } catch { status.value = 'Could not copy the image. Use Download PNG instead.' }
}
</script>
<style scoped>
.achievements{margin-top:2rem;padding-top:1.5rem;border-top:1px solid var(--ace-border)}h3{margin-bottom:.5rem}.achievements>p{font-size:.85rem;color:var(--ace-text-muted);line-height:1.6}.share-status{min-height:1.4em;color:var(--ace-secondary)!important}.badges{display:grid;grid-template-columns:1fr 1fr;gap:1rem}.badge-card{min-width:0;border:1px solid var(--ace-border-strong);border-radius:var(--ace-radius-md);padding:1rem}.badge{display:flex;gap:.75rem;align-items:center;padding:1rem;background:linear-gradient(135deg,rgba(193,211,1,.1),rgba(17,19,26,.9));border:1px solid var(--ace-primary);border-radius:var(--ace-radius-md)}.badge.mastery{border-color:#f5be3f}.badge-icon{color:var(--ace-primary);font-size:1.5rem}.badge small,.badge strong,.badge div>span{display:block}.badge small{font-size:.6rem;letter-spacing:.12em;color:var(--ace-text-muted)}.badge strong{font-size:.95rem;margin:.4rem 0}.badge div>span{font-size:.75rem;color:var(--ace-text-muted)}.badge-actions{display:flex;flex-wrap:wrap;gap:.5rem;margin-top:1rem}.badge-actions button{flex:1;font-size:.7rem}.locked .badge{opacity:.5}.locked p{font-size:.8rem;line-height:1.6}@media(max-width:720px){.badges{grid-template-columns:1fr}}
</style>
