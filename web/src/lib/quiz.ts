import bank from '../data/cdlQuizQuestions.json'

export interface QuizQuestion {
  id: string
  revision: number
  topic: string
  difficulty: string
  prompt: string
  options: { id: string; text: string }[]
  correctOptionId: string
  explanation: string
  reference: string
  referenceSection: string
  referenceBasis: string
}
export const quizQuestions: readonly QuizQuestion[] = bank
export const quizStorageKey = 'ace-cdl-quiz-v1'
export const quizLength = 15
const byId = new Map(quizQuestions.map(q => [q.id, q]))
export interface Attempt { choiceId: string; revision: number; answeredAt: string }
export function createQuizSessionId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `cdl-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 14)}`
}
export interface QuizSession {
  sessionId: string
  startedAt: string
  completedAt?: string
  participantEmail?: string
  deliveryStatus?: 'sending' | 'sent' | 'unavailable' | 'failed'
  deliveryAttemptedAt?: string
  questionIds: string[]
  revisions: number[]
  answers: Record<string, string>
  index: number
}
export interface QuizProgress {
  version: 1
  history: Record<string, Attempt[]>
  session: QuizSession | null
}
export function emptyQuizProgress(): QuizProgress { return { version: 1, history: {}, session: null } }
export function isCorrect(question: QuizQuestion, choiceId: string): boolean {
  return choiceId === question.correctOptionId
}
export function latestAttempt(progress: QuizProgress, question: QuizQuestion): Attempt | undefined {
  return progress.history[question.id]?.filter(a => a.revision === question.revision).at(-1)
}
function shuffled<T>(items: T[], random: () => number): T[] {
  const result = [...items]
  for (let i = result.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1))
    ;[result[i], result[j]] = [result[j]!, result[i]!]
  }
  return result
}
/** Unattempted first, then missed, then prior correct answers if needed to fill 15. */
export function selectQuizQuestions(progress: QuizProgress, random = Math.random): string[] {
  const unseen: QuizQuestion[] = [], missed: QuizQuestion[] = [], correct: QuizQuestion[] = []
  for (const question of quizQuestions) {
    const last = latestAttempt(progress, question)
    if (!last) unseen.push(question)
    else if (isCorrect(question, last.choiceId)) correct.push(question)
    else missed.push(question)
  }
  return [...shuffled(unseen, random), ...shuffled(missed, random), ...shuffled(correct, random)]
    .slice(0, quizLength).map(q => q.id)
}
export function startQuiz(progress: QuizProgress, random = Math.random): QuizProgress {
  // Keep an unfinished quiz stable across accidental repeated starts.
  if (progress.session && progress.session.index < progress.session.questionIds.length) return progress
  const questionIds = selectQuizQuestions(progress, random)
  return { ...progress, session: { sessionId: createQuizSessionId(), startedAt: new Date().toISOString(), questionIds, revisions: questionIds.map(id => byId.get(id)!.revision), answers: {}, index: 0 } }
}
export function answerQuiz(progress: QuizProgress, choiceId: string, answeredAt = new Date().toISOString()): QuizProgress {
  const session = progress.session
  const id = session?.questionIds[session.index]
  const question = id ? byId.get(id) : undefined
  if (!session || !id || !question || session.answers[id] || !question.options.some(o => o.id === choiceId)) return progress
  return { ...progress,
    history: { ...progress.history, [id]: [...(progress.history[id] ?? []), { choiceId, revision: question.revision, answeredAt }] },
    session: { ...session, answers: { ...session.answers, [id]: choiceId } },
  }
}
export function nextQuizQuestion(progress: QuizProgress): QuizProgress {
  const session = progress.session
  const id = session?.questionIds[session.index]
  if (!session || !id || !session.answers[id]) return progress
  return { ...progress, session: { ...session, index: session.index + 1, ...(session.index + 1 === session.questionIds.length ? { completedAt: new Date().toISOString() } : {}) } }
}
export function readQuizProgress(storage: Pick<Storage, 'getItem'>): { progress: QuizProgress; warning: string } {
  try {
    const raw = storage.getItem(quizStorageKey)
    if (!raw) return { progress: emptyQuizProgress(), warning: '' }
    const saved = JSON.parse(raw)
    if (saved?.version !== 1 || !saved.history || typeof saved.history !== 'object' || Array.isArray(saved.history)) throw new Error('Invalid quiz data')
    const progress = emptyQuizProgress()
    for (const question of quizQuestions) {
      const attempts = saved.history[question.id]
      if (!Array.isArray(attempts)) continue
      progress.history[question.id] = attempts.filter((a): a is Attempt => a &&
        a.revision === question.revision && question.options.some(o => o.id === a.choiceId) &&
        typeof a.answeredAt === 'string' && Number.isFinite(Date.parse(a.answeredAt)))
        .map(a => ({ choiceId: a.choiceId, revision: a.revision, answeredAt: a.answeredAt }))
    }
    const session = saved.session
    if (session && Array.isArray(session.questionIds) && session.questionIds.length === quizLength &&
        new Set(session.questionIds).size === quizLength && Array.isArray(session.revisions) &&
        session.revisions.length === quizLength && Number.isInteger(session.index) &&
        session.index >= 0 && session.index <= quizLength && session.answers && typeof session.answers === 'object' &&
        session.questionIds.every((id: unknown, index: number) => typeof id === 'string' &&
          byId.has(id) && session.revisions[index] === byId.get(id)!.revision)) {
      const ids = session.questionIds as string[]
      const valid = ids.every((id, index) => {
        const answer = session.answers[id]
        if (index < session.index) return byId.get(id)!.options.some(o => o.id === answer)
        if (index === session.index && answer !== undefined) return byId.get(id)!.options.some(o => o.id === answer)
        return answer === undefined
      })
      if (valid) progress.session = {
        sessionId: typeof session.sessionId === 'string' && /^[A-Za-z0-9-]{8,100}$/.test(session.sessionId) ? session.sessionId : createQuizSessionId(),
        startedAt: typeof session.startedAt === 'string' && Number.isFinite(Date.parse(session.startedAt)) ? session.startedAt : new Date().toISOString(),
        ...(session.index === quizLength && typeof session.completedAt === 'string' && Number.isFinite(Date.parse(session.completedAt)) ? { completedAt: session.completedAt } : {}),
        ...(typeof session.participantEmail === 'string' ? { participantEmail: session.participantEmail } : {}),
        ...(['sent', 'failed', 'unavailable', 'sending'].includes(session.deliveryStatus) ? { deliveryStatus: session.deliveryStatus === 'sending' ? 'failed' : session.deliveryStatus } : {}),
        ...(typeof session.deliveryAttemptedAt === 'string' && Number.isFinite(Date.parse(session.deliveryAttemptedAt)) ? { deliveryAttemptedAt: session.deliveryAttemptedAt } : {}),
        questionIds: ids, revisions: [...session.revisions], index: session.index,
        answers: Object.fromEntries(ids.filter(id => session.answers[id] !== undefined).map(id => [id, session.answers[id]])) }
    }
    return { progress, warning: '' }
  } catch {
    return { progress: emptyQuizProgress(), warning: 'Saved quiz progress could not be read. You can still take a quiz in this tab.' }
  }
}
export function saveQuizProgress(storage: Pick<Storage, 'setItem'>, progress: QuizProgress): boolean {
  try { storage.setItem(quizStorageKey, JSON.stringify(progress)); return true } catch { return false }
}
