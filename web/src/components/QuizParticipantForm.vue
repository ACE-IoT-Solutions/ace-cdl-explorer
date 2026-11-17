<template>
  <section class="participant-card">
    <span class="eyebrow">SAVE YOUR RESULTS</span>
    <h2>Enter your email to begin</h2>
    <p v-if="remoteConfigured">We use your email to save completed quiz results to the CDL Explorer learning record and, when connected, the associated Pipedrive contact. This does not subscribe you to marketing.</p>
    <p v-else>Results collection is not connected here. Your email and quiz progress stay in this browser.</p>
    <form @submit.prevent="register">
      <label class="field"><span>Email address</span><input v-model.trim="email" type="email" required maxlength="254" autocomplete="email" placeholder="you@example.com" @input="error = ''" /></label>
      <label class="consent"><input v-model="accepted" type="checkbox" required /><span>I agree to storage of my email and quiz results as described in the <a :href="quizPrivacyPolicyUrl" target="_blank" rel="noreferrer">privacy policy</a>.</span></label>
      <label class="honeypot" aria-hidden="true">Website<input v-model="website" type="text" tabindex="-1" autocomplete="off" /></label>
      <p v-if="error" role="alert">{{ error }}</p>
      <button class="solid-button" type="submit">Start competency quiz</button>
    </form>
  </section>
</template>
<script setup lang="ts">
import { ref } from 'vue'
import { isQuizResultsEndpointConfigured, isValidQuizEmail, normalizeQuizEmail, quizPrivacyPolicyUrl, type QuizParticipant } from '../lib/quiz-results'
const props = defineProps<{ initialEmail?: string }>()
const emit = defineEmits<{ register: [participant: QuizParticipant] }>()
const email = ref(props.initialEmail ?? '')
const accepted = ref(false), website = ref(''), error = ref('')
const remoteConfigured = isQuizResultsEndpointConfigured()
function register() {
  if (website.value) return
  if (!isValidQuizEmail(email.value)) { error.value = 'Enter a valid email address.'; return }
  if (!accepted.value) { error.value = 'Accept the privacy policy to continue.'; return }
  emit('register', { version: 1, email: normalizeQuizEmail(email.value), consentAt: new Date().toISOString(), remoteConsent: remoteConfigured })
}
</script>
<style scoped>
.participant-card{padding:2rem;margin:2rem 0;background:var(--ace-bg-panel);border:1px solid var(--ace-border-strong);border-radius:var(--ace-radius-md)}h2{margin:.75rem 0}p{line-height:1.7;color:var(--ace-text-muted)}form{display:grid;gap:1.25rem;max-width:650px}.consent{display:flex;gap:.75rem;font-size:.9rem;line-height:1.6}.consent input{align-self:start;margin-top:.4rem;accent-color:var(--ace-primary)}a{color:var(--ace-secondary)}.honeypot{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%)}button{justify-self:start}
</style>
