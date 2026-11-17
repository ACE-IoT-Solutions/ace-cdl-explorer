import { isCorrect, quizLength, quizQuestions, type QuizSession } from './quiz'

// Same ACE IoT policy used by BACnet Studio's learning-record workflow.
export const quizPrivacyPolicyUrl = 'https://www.privacypolicyonline.com/live.php?token=IuZqxCYZLiM2psa7Bf3mOhJ1tESPxPQU'
export const quizParticipantKey = 'ace-cdl-quiz-participant-v1'
export interface QuizParticipant { version: 1; email: string; consentAt: string; remoteConsent?: boolean }
export type DeliveryStatus = 'sending' | 'sent' | 'unavailable' | 'failed'
export const quizResultsEndpoint = typeof __QUIZ_RESULTS_ENDPOINT__ === 'string' ? __QUIZ_RESULTS_ENDPOINT__ : ''
export function normalizeQuizEmail(value: string) { return value.trim().toLowerCase() }
export function isValidQuizEmail(value: string): boolean {
  const email = normalizeQuizEmail(value)
  return email.length <= 254 && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)
}
export function readQuizParticipant(storage: Pick<Storage, 'getItem'>, requiresRemoteConsent = isQuizResultsEndpointConfigured()): QuizParticipant | null {
  try {
    const value = JSON.parse(storage.getItem(quizParticipantKey) ?? 'null')
    if (value?.version !== 1 || typeof value.email !== 'string' || !isValidQuizEmail(value.email) ||
        typeof value.consentAt !== 'string' || !Number.isFinite(Date.parse(value.consentAt))) return null
    if (requiresRemoteConsent && value.remoteConsent !== true) return null
    return { version: 1, email: normalizeQuizEmail(value.email), consentAt: value.consentAt, remoteConsent: value.remoteConsent === true }
  } catch { return null }
}
export function isQuizResultsEndpointConfigured(endpoint = quizResultsEndpoint): boolean {
  return /^https:\/\/script\.google\.com\/macros\/s\/[A-Za-z0-9_-]+\/exec$/.test(endpoint)
}
export interface QuizResultPayload {
  version: 1
  appId: 'ace-cdl-explorer'
  sessionId: string
  email: string
  consentAt: string
  startedAt: string
  completedAt: string
  score: number
  total: number
  percentage: number
  masteredTotal: number
  bankVersion: number
  questionIds: string[]
  questionRevisions: number[]
  answers: Record<string, string>
  incorrectQuestionIds: string[]
  privacyPolicyUrl: string
  sourceUrl: string
  website: string
}
export function buildQuizResultPayload(session: QuizSession, participant: QuizParticipant, masteredTotal: number, sourceUrl: string): QuizResultPayload {
  if (!session.completedAt || session.index !== quizLength || session.questionIds.length !== quizLength ||
      new Set(session.questionIds).size !== quizLength) throw new Error('Cannot submit an incomplete quiz.')
  if (!isValidQuizEmail(participant.email) || session.participantEmail !== normalizeQuizEmail(participant.email) ||
      !Number.isFinite(Date.parse(participant.consentAt))) throw new Error('Quiz participant consent does not match this round.')
  const byId = new Map(quizQuestions.map(q => [q.id, q]))
  for (const [index, id] of session.questionIds.entries()) {
    const q = byId.get(id)
    if (!q || q.revision !== session.revisions[index] || !q.options.some(o => o.id === session.answers[id])) throw new Error('Invalid quiz answer or question revision.')
  }
  const incorrectQuestionIds = session.questionIds.filter(id => !isCorrect(byId.get(id)!, session.answers[id]!))
  const score = session.questionIds.length - incorrectQuestionIds.length
  // Do not send query parameters, saved project identifiers or arbitrary hash content.
  const source = new URL(sourceUrl)
  source.search = ''
  source.hash = 'quiz'
  return {
    version: 1, appId: 'ace-cdl-explorer', sessionId: session.sessionId,
    email: normalizeQuizEmail(participant.email), consentAt: participant.consentAt,
    startedAt: session.startedAt, completedAt: session.completedAt,
    score, total: quizLength, percentage: Number((score / quizLength * 100).toFixed(1)), masteredTotal,
    bankVersion: Math.max(...quizQuestions.map(q => q.revision)), questionIds: [...session.questionIds],
    questionRevisions: [...session.revisions], answers: Object.fromEntries(session.questionIds.map(id => [id, session.answers[id]!])),
    incorrectQuestionIds, privacyPolicyUrl: quizPrivacyPolicyUrl, sourceUrl: source.toString(), website: '',
  }
}
export async function submitQuizResult(payload: QuizResultPayload, endpoint = quizResultsEndpoint): Promise<Exclude<DeliveryStatus, 'sending'>> {
  if (!isQuizResultsEndpointConfigured(endpoint)) return 'unavailable'
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 15_000)
  try {
    // Apps Script responds across origins. An opaque response confirms dispatch,
    // not the receiver's success; the UI says "submitted for processing".
    await fetch(endpoint, { method: 'POST', mode: 'no-cors', body: new URLSearchParams({ payload: JSON.stringify(payload) }), keepalive: true, signal: controller.signal })
    return 'sent'
  } catch { return 'failed' } finally { clearTimeout(timeout) }
}
