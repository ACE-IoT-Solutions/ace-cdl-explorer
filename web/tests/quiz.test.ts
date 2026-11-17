import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  answerQuiz, emptyQuizProgress, isCorrect, latestAttempt, nextQuizQuestion,
  quizLength, quizQuestions, quizStorageKey, readQuizProgress,
  saveQuizProgress, selectQuizQuestions, startQuiz,
} from '../src/lib/quiz'

const fixedRandom = () => 0.37
const memoryStorage = () => {
  const values = new Map<string, string>()
  return { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value) } }
}
function practiced(correctCount: number, missedCount = 0) {
  const progress = emptyQuizProgress()
  for (const [index, q] of quizQuestions.entries()) {
    if (index >= correctCount + missedCount) break
    progress.history[q.id] = [{ choiceId: index < correctCount ? q.correctOptionId : q.options.find(o => o.id !== q.correctOptionId)!.id,
      revision: q.revision, answeredAt: '2026-11-16T12:00:00.000Z' }]
  }
  return progress
}

test('bank has fifty stable, distinct, fully answerable questions', () => {
  assert.equal(quizQuestions.length, 50)
  assert.equal(new Set(quizQuestions.map(q => q.id)).size, 50)
  assert.equal(new Set(quizQuestions.map(q => q.prompt)).size, 50)
  for (const q of quizQuestions) {
    assert.equal(q.options.length, 4)
    assert.equal(new Set(q.options.map(o => o.id)).size, 4)
    assert.equal(new Set(q.options.map(o => o.text)).size, 4)
    assert.equal(q.options.filter(o => isCorrect(q, o.id)).length, 1)
    assert.ok(q.explanation && q.reference && q.topic && q.revision > 0)
  }
})
test('a new quiz samples fifteen unique questions and different random draws vary', () => {
  const first = selectQuizQuestions(emptyQuizProgress(), fixedRandom)
  assert.equal(first.length, quizLength)
  assert.equal(new Set(first).size, quizLength)
  assert.notDeepEqual(first, selectQuizQuestions(emptyQuizProgress(), () => 0.8))
})
test('never-attempted questions take priority over missed and previously correct answers', () => {
  const progress = practiced(20, 15)
  const ids = selectQuizQuestions(progress, fixedRandom)
  assert.equal(ids.length, 15)
  assert.ok(ids.every(id => !progress.history[id]))
})
test('missed answers fill after unseen questions; prior correct answers are excluded while possible', () => {
  const progress = practiced(30, 15)
  const ids = selectQuizQuestions(progress, fixedRandom)
  assert.ok(ids.slice(0, 5).every(id => !progress.history[id]))
  assert.ok(ids.slice(5).every(id => {
    const q = quizQuestions.find(q => q.id === id)!
    return !isCorrect(q, latestAttempt(progress, q)!.choiceId)
  }))
  assert.ok(ids.every(id => !quizQuestions.slice(0, 30).some(q => q.id === id)))
})
test('a nearly exhausted bank fills to fifteen with review only after remaining questions', () => {
  const progress = practiced(45, 2)
  const ids = selectQuizQuestions(progress, fixedRandom)
  assert.ok(ids.slice(0, 3).every(id => !progress.history[id]))
  assert.ok(ids.slice(3, 5).every(id => quizQuestions.slice(45, 47).some(q => q.id === id)))
  assert.ok(ids.slice(5).every(id => quizQuestions.slice(0, 45).some(q => q.id === id)))
  assert.equal(new Set(ids).size, 15)
})
test('a fully correct bank starts a review quiz without clearing history', () => {
  const progress = practiced(50)
  const next = startQuiz(progress, fixedRandom)
  assert.equal(next.session!.questionIds.length, 15)
  assert.deepEqual(next.history, progress.history)
})
test('answers are recorded once and advancement requires a checked answer', () => {
  const progress = startQuiz(emptyQuizProgress(), fixedRandom)
  assert.equal(nextQuizQuestion(progress), progress)
  assert.equal(startQuiz(progress), progress)
  assert.equal(answerQuiz(progress, 'INVALID'), progress)
  const id = progress.session!.questionIds[0]!
  const q = quizQuestions.find(q => q.id === id)!
  const checked = answerQuiz(progress, q.correctOptionId)
  assert.equal(checked.history[id]!.length, 1)
  assert.equal(answerQuiz(checked, 'B'), checked)
  assert.equal(nextQuizQuestion(checked).session!.index, 1)
})
test('local storage restores checked answers, active position and completed scores', () => {
  const storage = memoryStorage()
  let progress = startQuiz(emptyQuizProgress(), fixedRandom)
  for (let i = 0; i < quizLength; i++) {
    const q = quizQuestions.find(q => q.id === progress.session!.questionIds[i])!
    progress = answerQuiz(progress, q.correctOptionId)
    assert.equal(saveQuizProgress(storage, progress), true)
    const restored = readQuizProgress(storage)
    assert.equal(restored.warning, '')
    assert.deepEqual(restored.progress, progress)
    progress = nextQuizQuestion(restored.progress)
  }
  saveQuizProgress(storage, progress)
  assert.equal(readQuizProgress(storage).progress.session!.index, 15)
  const next = startQuiz(readQuizProgress(storage).progress, fixedRandom)
  assert.ok(next.session!.questionIds.every(id => !progress.history[id]))
})
test('missed review answers return to practice', () => {
  const progress = startQuiz(practiced(50), fixedRandom)
  const id = progress.session!.questionIds[0]!
  const q = quizQuestions.find(q => q.id === id)!
  const next = answerQuiz(progress, q.options.find(o => o.id !== q.correctOptionId)!.id)
  assert.equal(next.history[id]!.length, 2)
  assert.equal(selectQuizQuestions(next, fixedRandom)[0], id)
})
test('invalid, obsolete and unavailable storage does not break a quiz', () => {
  for (const raw of ['{', 'null', '{"version":2,"history":{}}']) {
    const result = readQuizProgress({ getItem: () => raw })
    assert.deepEqual(result.progress, emptyQuizProgress())
    assert.ok(result.warning)
  }
  assert.ok(readQuizProgress({ getItem: () => { throw new Error('denied') } }).warning)
  assert.equal(saveQuizProgress({ setItem: () => { throw new Error('full') } }, emptyQuizProgress()), false)
})
test('changed question revisions invalidate old answers and the active session', () => {
  const storage = memoryStorage()
  let progress = startQuiz(emptyQuizProgress(), fixedRandom)
  const q = quizQuestions.find(q => q.id === progress.session!.questionIds[0])!
  progress = answerQuiz(progress, q.correctOptionId)
  progress.history[q.id]![0]!.revision = 0
  progress.session!.revisions[0] = 0
  saveQuizProgress(storage, progress)
  const restored = readQuizProgress(storage).progress
  assert.equal(latestAttempt(restored, q), undefined)
  assert.equal(restored.session, null)
})
test('corrupt session IDs are discarded while valid answer history is retained', () => {
  const storage = memoryStorage()
  const progress = startQuiz(practiced(10), fixedRandom)
  progress.session!.questionIds[1] = progress.session!.questionIds[0]!
  storage.setItem(quizStorageKey, JSON.stringify(progress))
  const restored = readQuizProgress(storage).progress
  assert.equal(restored.session, null)
  assert.equal(Object.values(restored.history).filter(h => h.length).length, 10)
})

test('question content is standards-focused and references primary sources', () => {
  const allowedHosts = new Set(['data.ashrae.org', 'store.accuristech.com', 'obc.lbl.gov', 'specification.modelica.org', 'github.com'])
  for (const q of quizQuestions) {
    assert.ok(q.revision >= 2)
    assert.ok(q.referenceSection && q.referenceBasis)
    const source = new URL(q.reference)
    assert.ok(allowedHosts.has(source.hostname))
    if (source.hostname === 'github.com') {
      assert.ok(source.pathname.startsWith('/lbl-srg/modelica-buildings/blob/v12.1.0/Buildings/Controls/OBC/CDL/'))
    }
    assert.doesNotMatch(q.prompt + ' ' + q.explanation, /Explorer|this project|checkpoint|handover|import.only|weather|local.storage/i)
  }
  const add = quizQuestions.find(q => q.prompt.startsWith('Reals.Add receives'))!
  assert.equal(add.options.find(o => o.id === add.correctOptionId)!.text, '7')
  assert.doesNotMatch(add.prompt, /k1|k2/)
})
