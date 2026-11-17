<template>
  <div class="quiz-page">
    <section class="quiz-hero">
      <span class="eyebrow">PRACTICE / ASHRAE STANDARD 231</span>
      <h1>Put your CDL knowledge to work.</h1>
      <p>Fifteen questions at a time, from a bank of fifty. Practice CDL language rules, abstract signal interfaces and standardized block behavior.</p>
      <p class="muted">New questions come first, followed by questions you missed. Correct answers stay out of the pool until the remaining questions can’t fill a quiz. Review questions then return; a missed review answer becomes practice again.</p>
      <div class="quiz-stats" aria-label="Saved learning progress">
        <div><strong>{{ attemptedCount }} / 50</strong><span>Questions attempted</span></div>
        <div><strong>{{ correctCount }} / 50</strong><span>Latest answer correct</span></div>
        <div><strong>15</strong><span>Questions per quiz</span></div>
      </div>
      <label class="quiz-progress-label">Bank progress <progress :value="correctCount" max="50">{{ correctCount }} of 50</progress></label>
      <p class="muted small">Answers and your current quiz are saved on this browser. You can leave this tab and return.</p>
      <p v-if="storageWarning" class="quiz-warning" role="status">{{ storageWarning }}</p>
    </section>

    <p v-if="participant" class="quiz-participant">Learning record: {{ participant.email }} <button class="text-button" @click="changeEmail">Change email</button></p>
    <QuizParticipantForm v-if="!participant" :initial-email="previousEmail" @register="registerParticipant" />
    <section v-else-if="!session" class="quiz-card quiz-start">
      <h2>Ready for a round?</h2>
      <p>Choose one answer, then check it to see the explanation. There’s no time limit.</p>
      <button class="solid-button" @click="begin">Start 15-question quiz</button>
    </section>

    <section v-else-if="currentQuestion" class="quiz-card" aria-label="Quiz question">
      <div class="quiz-question-meta"><span>Question {{ session.index + 1 }} of {{ session.questionIds.length }}</span><span>{{ currentQuestion.topic }} · {{ currentQuestion.difficulty }}</span></div>
      <h2 ref="stepHeading" tabindex="-1">{{ currentQuestion.prompt }}</h2>
      <form @submit.prevent="checkAnswer">
        <fieldset :disabled="Boolean(currentAnswer)">
          <legend class="sr-only">Choose one answer</legend>
          <label v-for="option in currentQuestion.options" :key="option.id" class="quiz-option" :class="{
            selected: selected === option.id,
            correct: currentAnswer && option.id === currentQuestion.correctOptionId,
            incorrect: currentAnswer === option.id && !isCorrect(currentQuestion, option.id),
          }">
            <input v-model="selected" type="radio" name="quiz-answer" :value="option.id" />
            <span class="quiz-option-letter">{{ option.id }}</span><span>{{ option.text }}</span>
          </label>
        </fieldset>
        <button v-if="!currentAnswer" class="solid-button" type="submit" :disabled="!selected">Check answer</button>
      </form>
      <div v-if="currentAnswer" class="quiz-feedback" role="status">
        <h3>{{ isCorrect(currentQuestion, currentAnswer) ? 'Correct.' : 'Not quite.' }}</h3>
        <p><strong>Correct answer: {{ currentQuestion.correctOptionId }}.</strong> {{ answerText(currentQuestion, currentQuestion.correctOptionId) }}</p>
        <p>{{ currentQuestion.explanation }}</p>
        <button class="solid-button" @click="advance">{{ session.index === session.questionIds.length - 1 ? 'See results' : 'Next question' }}</button>
      </div>
    </section>

    <section v-else-if="session" class="quiz-card quiz-results">
      <span class="eyebrow">ROUND COMPLETE</span>
      <h2 ref="stepHeading" tabindex="-1">{{ score }} of {{ session.questionIds.length }} correct.</h2>
      <p>Correct answers are saved. Your next round prioritizes new questions and missed answers.</p>
      <div class="quiz-delivery" role="status">
        <p v-if="session.deliveryStatus==='sent'">Results submitted for processing for {{ participant?.email }}.</p>
        <p v-else-if="session.deliveryStatus==='failed'">Results are saved in this browser, but could not be sent.</p>
        <p v-else-if="session.deliveryStatus==='unavailable' || !session.completedAt">Results are saved in this browser. Remote collection is not connected here.</p>
        <p v-else>Submitting results…</p>
        <button v-if="session.deliveryStatus==='failed' || session.deliveryStatus==='unavailable'" class="text-button" @click="deliverResult">Try again</button>
      </div>
      <QuizAchievements :key="session.sessionId" :score="score" :total="session.questionIds.length" :mastered-total="masteredCount" :bank-total="quizQuestions.length" />
      <button class="solid-button" @click="begin">Start another 15-question quiz</button>
      <details class="quiz-review"><summary>Review this round’s answers</summary>
        <ol><li v-for="question in roundQuestions" :key="question.id">
          <h3>{{ question.prompt }}</h3>
          <p><strong>Your answer:</strong> {{ answerText(question, session.answers[question.id] ?? '') }}</p>
          <p><strong>Correct answer:</strong> {{ answerText(question, question.correctOptionId) }}</p>
          <p class="muted">{{ question.explanation }}</p>
        </li></ol>
      </details>
    </section>

    <section class="quiz-footer">
      <a href="https://data.ashrae.org/standard231/" target="_blank" rel="noreferrer">Standard 231 resources ↗</a>
      <button v-if="!confirmReset" class="text-button" @click="confirmReset = true">Reset quiz progress</button>
      <div v-else class="quiz-reset" role="group" aria-label="Confirm progress reset">
        <p>Clear all quiz answers and the current round on this browser?</p>
        <button class="outline-button" @click="confirmReset = false">Keep progress</button>
        <button class="outline-button" @click="reset">Clear quiz progress</button>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import QuizParticipantForm from './QuizParticipantForm.vue'
import QuizAchievements from './QuizAchievements.vue'
import { buildQuizResultPayload, quizParticipantKey, readQuizParticipant, submitQuizResult, type DeliveryStatus, type QuizParticipant } from '../lib/quiz-results'
import {
  answerQuiz, emptyQuizProgress, isCorrect, latestAttempt, nextQuizQuestion,
  quizQuestions, readQuizProgress, saveQuizProgress, startQuiz,
  type QuizProgress, type QuizQuestion,
} from '../lib/quiz'

let storage: Storage | undefined
let initial = { progress: emptyQuizProgress(), warning: '' }
try { storage = window.localStorage; initial = readQuizProgress(storage) }
catch { initial.warning = 'Browser storage is unavailable. You can take a quiz, but progress will last only while this tab stays open.' }
const progress = ref<QuizProgress>(initial.progress)
const storageWarning = ref(initial.warning)
const participant = ref<QuizParticipant | null>(storage ? readQuizParticipant(storage) : null)
const previousEmail = ref(participant.value?.email ?? '')
const inFlight = new Set<string>()
let componentActive = true
onUnmounted(() => { componentActive = false })
const session = computed(() => progress.value.session)
const questionsById = new Map(quizQuestions.map(q => [q.id, q]))
const currentQuestion = computed(() => {
  const id = session.value?.questionIds[session.value.index]
  return id ? questionsById.get(id) : undefined
})
const currentAnswer = computed(() => currentQuestion.value ? session.value?.answers[currentQuestion.value.id] : undefined)
const selected = ref(currentAnswer.value ?? '')
const confirmReset = ref(false)
const stepHeading = ref<HTMLElement>()
const attemptedCount = computed(() => quizQuestions.filter(q => latestAttempt(progress.value, q)).length)
const correctCount = computed(() => quizQuestions.filter(q => {
  const attempt = latestAttempt(progress.value, q)
  return attempt && isCorrect(q, attempt.choiceId)
}).length)
const masteredCount = computed(() => quizQuestions.filter(q => progress.value.history[q.id]?.some(a => a.revision === q.revision && isCorrect(q, a.choiceId))).length)
const roundQuestions = computed(() => session.value?.questionIds.map(id => questionsById.get(id)!) ?? [])
const score = computed(() => roundQuestions.value.filter(q => isCorrect(q, session.value?.answers[q.id] ?? '')).length)
function answerText(question: QuizQuestion, id: string) { return question.options.find(o => o.id === id)?.text ?? 'No answer' }
function persist(next: QuizProgress) {
  progress.value = next
  storageWarning.value = storage && saveQuizProgress(storage, next) ? '' :
    'Progress could not be saved on this browser. This quiz still works, but changes may be lost when you leave or reload.'
}
async function focusStep() { await nextTick(); stepHeading.value?.focus() }
function begin() {
  if (!participant.value) return
  const next = startQuiz(progress.value)
  if (next.session) next.session = { ...next.session, participantEmail: participant.value.email }
  persist(next); selected.value = ''; void focusStep()
}
function registerParticipant(value: QuizParticipant) {
  participant.value = value; previousEmail.value = value.email
  try { if (storage) storage.setItem(quizParticipantKey, JSON.stringify(value)) } catch { storageWarning.value = 'Your email could not be saved on this browser.' }
  const current = progress.value.session
  // A different participant starts a new round; completed anonymous rounds are
  // never retroactively transmitted under a newly supplied email.
  if (current && ((current.participantEmail && current.participantEmail !== value.email) ||
      (current.index === current.questionIds.length && !current.participantEmail))) {
    progress.value = { ...progress.value, session: null }
  }
  begin()
}
function changeEmail() {
  previousEmail.value = participant.value?.email ?? ''
  participant.value = null
  try { storage?.removeItem(quizParticipantKey) } catch { storageWarning.value = 'The saved email could not be cleared on this browser.' }
}
function setDelivery(id: string, status: DeliveryStatus, attemptedAt?: string) {
  if (!componentActive || progress.value.session?.sessionId !== id) return
  persist({ ...progress.value, session: { ...progress.value.session, deliveryStatus: status,
    ...(attemptedAt ? { deliveryAttemptedAt: attemptedAt } : {}) } })
}
async function deliverResult() {
  const round = session.value, person = participant.value
  if (!round?.completedAt || !person || round.participantEmail !== person.email || inFlight.has(round.sessionId)) return
  const id = round.sessionId
  inFlight.add(id)
  setDelivery(id, 'sending', new Date().toISOString())
  try { setDelivery(id, await submitQuizResult(buildQuizResultPayload(round, person, masteredCount.value, location.href))) }
  catch { setDelivery(id, 'failed') }
  finally { inFlight.delete(id) }
}
watch([() => session.value?.completedAt, () => participant.value?.email], () => {
  if (session.value?.completedAt && participant.value && session.value.participantEmail === participant.value.email && !session.value.deliveryAttemptedAt) void deliverResult()
}, { immediate: true })
function checkAnswer() { if (selected.value) persist(answerQuiz(progress.value, selected.value)) }
function advance() { persist(nextQuizQuestion(progress.value)); selected.value = ''; void focusStep() }
function reset() { persist(emptyQuizProgress()); selected.value = ''; confirmReset.value = false }
</script>

<style scoped>
.quiz-results > .solid-button{margin-top:1.5rem}.quiz-delivery{overflow-wrap:anywhere;padding:1rem;margin:1rem 0;border:1px solid var(--ace-border);border-radius:var(--ace-radius-md)}.quiz-delivery p{margin:0 0 .5rem}.quiz-participant{font-size:.85rem;color:var(--ace-text-muted);overflow-wrap:anywhere}.quiz-page{max-width:1050px;margin:auto;padding:3rem clamp(1.25rem,4vw,3rem) 4rem}.quiz-hero h1{font-size:clamp(2.3rem,5vw,4rem);margin:1rem 0}.quiz-hero p{max-width:780px;line-height:1.75}.quiz-stats{display:grid;grid-template-columns:repeat(3,1fr);gap:1rem;margin:2rem 0}.quiz-stats div{display:flex;flex-direction:column;gap:.4rem;padding:1rem;border:1px solid var(--ace-border);border-radius:var(--ace-radius-md);background:var(--ace-bg-panel)}.quiz-stats strong{font-size:1.8rem;color:var(--ace-primary)}.quiz-stats span,.quiz-progress-label{font-size:.8rem;color:var(--ace-text-muted)}.quiz-progress-label{display:flex;align-items:center;gap:1rem}.quiz-progress-label progress{flex:1;accent-color:var(--ace-primary);height:.65rem}.quiz-card{margin:2rem 0;padding:clamp(1.25rem,3vw,2rem);background:var(--ace-bg-panel);border:1px solid var(--ace-border-strong);border-radius:var(--ace-radius-md)}.quiz-card h2{font-size:clamp(1.25rem,2.5vw,1.8rem);line-height:1.4}.quiz-question-meta{display:flex;justify-content:space-between;flex-wrap:wrap;gap:.5rem;font-size:.78rem;color:var(--ace-text-muted);margin-bottom:1.25rem}.quiz-card fieldset{border:0;padding:0;margin:1.5rem 0;display:grid;gap:.75rem;min-width:0}.quiz-option{display:flex;align-items:center;gap:.75rem;padding:1rem;border:1px solid var(--ace-border-strong);border-radius:var(--ace-radius-md);cursor:pointer;line-height:1.5}.quiz-option.selected{border-color:var(--ace-secondary);background:rgba(66,200,193,.06)}.quiz-option.correct{border-color:var(--ace-primary)}.quiz-option.incorrect{border-color:#f1a58f}.quiz-option:focus-within{outline:2px solid var(--ace-secondary);outline-offset:3px}.quiz-option input{accent-color:var(--ace-primary);flex-shrink:0}.quiz-option-letter{color:var(--ace-text-muted);font-weight:700}.quiz-feedback{margin-top:1.5rem;border-top:1px solid var(--ace-border);padding-top:1.5rem}.quiz-feedback p,.quiz-results p{line-height:1.7}.quiz-feedback h3{color:var(--ace-primary)}.quiz-footer{display:flex;align-items:start;justify-content:space-between;gap:1rem;flex-wrap:wrap}.quiz-footer a{color:var(--ace-secondary);font-size:.85rem}.quiz-warning{padding:1rem;border:1px solid #f1a58f;border-radius:var(--ace-radius-md)}.quiz-reset p{font-size:.85rem}.quiz-reset button+button{margin-left:.5rem}.quiz-review{margin-top:2rem}.quiz-review summary{cursor:pointer;color:var(--ace-secondary)}.quiz-review ol{padding-left:1.4rem}.quiz-review li{margin:1.5rem 0;padding-bottom:1rem;border-bottom:1px solid var(--ace-border)}.quiz-review h3{font-size:1rem;line-height:1.5}.quiz-review p{font-size:.9rem}@media(max-width:600px){.quiz-stats{grid-template-columns:1fr}.quiz-stats div{flex-direction:row;align-items:center;gap:1rem}.quiz-option{padding:.8rem}.quiz-question-meta{flex-direction:column}}
</style>
