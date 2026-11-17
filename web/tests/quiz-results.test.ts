import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync } from 'node:fs'
import { runInNewContext } from 'node:vm'
import { answerQuiz, emptyQuizProgress, nextQuizQuestion, quizQuestions, readQuizProgress, saveQuizProgress, startQuiz } from '../src/lib/quiz'
import { buildQuizResultPayload, isQuizResultsEndpointConfigured, isValidQuizEmail, normalizeQuizEmail, quizParticipantKey, readQuizParticipant, submitQuizResult } from '../src/lib/quiz-results'
import { badgeText, canonicalQuizUrl, quizShareUrl } from '../src/lib/quiz-social'
// @ts-ignore JavaScript configuration is shared with Vite.
import { quizResultsEndpointForBuild } from '../build-config.mjs'
const endpoint = 'https://script.google.com/macros/s/test-deployment/exec'
function completed() {
  let progress = startQuiz(emptyQuizProgress(), () => 0.5)
  progress.session!.participantEmail = 'learner@example.com'
  for (let i = 0; i < 15; i++) {
    const q = quizQuestions.find(q => q.id === progress.session!.questionIds[i])!
    progress = nextQuizQuestion(answerQuiz(progress, i === 0 ? q.options.find(o => o.id !== q.correctOptionId)!.id : q.correctOptionId))
  }
  return progress
}
const person = { version: 1 as const, email: ' Learner@Example.com ', consentAt: '2026-01-01T00:00:00.000Z' }
function payload() { return buildQuizResultPayload(completed().session!, person, 30, 'https://example.com/cdl/?private=discard#other') }

test('participant validation requires a valid email and explicit consent timestamp', () => {
  assert.equal(normalizeQuizEmail(person.email), 'learner@example.com')
  assert.ok(isValidQuizEmail(person.email))
  assert.equal(isValidQuizEmail('not-an-email'), false)
  assert.equal(isValidQuizEmail(`${'a'.repeat(255)}@example.com`), false)
  assert.equal(readQuizParticipant({ getItem: () => JSON.stringify({ email: person.email }) }), null)
  assert.equal(readQuizParticipant({ getItem: () => JSON.stringify({ ...person, consentAt: 'invalid' }) }), null)
  assert.equal(readQuizParticipant({ getItem: key => key === quizParticipantKey ? JSON.stringify(person) : null })!.email, 'learner@example.com')
  assert.equal(readQuizParticipant({ getItem: () => JSON.stringify(person) }, true), null)
  assert.ok(readQuizParticipant({ getItem: () => JSON.stringify({ ...person, remoteConsent: true }) }, true))
})
test('completed payload contains fifteen answers, actual score, consent and source without query data', () => {
  const result = payload()
  assert.equal(result.score, 14)
  assert.equal(result.percentage, 93.3)
  assert.equal(result.total, 15)
  assert.equal(result.appId, 'ace-cdl-explorer')
  assert.equal(result.email, 'learner@example.com')
  assert.equal(result.sourceUrl, 'https://example.com/cdl/#quiz')
  assert.equal(result.incorrectQuestionIds.length, 1)
  assert.equal(Object.keys(result.answers).length, 15)
  assert.ok(result.questionRevisions.every(r => r === 2))
  assert.throws(() => buildQuizResultPayload(startQuiz(emptyQuizProgress()).session!, person, 0, canonicalQuizUrl), /incomplete/)
  assert.throws(() => buildQuizResultPayload(completed().session!, { ...person, email: 'another@example.com' }, 0, canonicalQuizUrl), /consent/)
})
test('delivery metadata and session IDs persist; interrupted delivery becomes retryable', () => {
  const progress = completed()
  progress.session!.deliveryStatus = 'sending'
  progress.session!.deliveryAttemptedAt = new Date().toISOString()
  let raw = ''
  saveQuizProgress({ setItem: (_, value) => { raw = value } }, progress)
  const restored = readQuizProgress({ getItem: () => raw }).progress
  assert.equal(restored.session!.sessionId, progress.session!.sessionId)
  assert.equal(restored.session!.completedAt, progress.session!.completedAt)
  assert.equal(restored.session!.deliveryStatus, 'failed')
})
test('canonical production builds alone receive an Apps Script endpoint', () => {
  const env = { CI: 'true', GITHUB_REPOSITORY: 'ACE-IoT-Solutions/ace-cdl-explorer', QUIZ_RESULTS_ENDPOINT: endpoint }
  assert.equal(quizResultsEndpointForBuild('build', env), endpoint)
  assert.equal(quizResultsEndpointForBuild('serve', env), '')
  assert.equal(quizResultsEndpointForBuild('build', { ...env, CI: '' }), '')
  assert.equal(quizResultsEndpointForBuild('build', { ...env, GITHUB_REPOSITORY: 'a/fork' }), '')
  assert.equal(quizResultsEndpointForBuild('build', { ...env, QUIZ_RESULTS_ENDPOINT: 'https://example.com/exec' }), '')
  assert.equal(isQuizResultsEndpointConfigured(endpoint + '?other=1'), false)
})
test('requests are form encoded, disabled when unconfigured and network failures remain retryable', async t => {
  const calls: RequestInit[] = []
  t.mock.method(globalThis, 'fetch', async (_url, init) => { calls.push(init); return new Response() })
  assert.equal(await submitQuizResult(payload(), ''), 'unavailable')
  assert.equal(calls.length, 0)
  const result = payload()
  assert.equal(await submitQuizResult(result, endpoint), 'sent')
  assert.equal(calls[0]!.mode, 'no-cors')
  assert.equal(calls[0]!.keepalive, true)
  assert.deepEqual(JSON.parse((calls[0]!.body as URLSearchParams).get('payload')!), result)
})
test('network errors return a failed delivery status without changing stored answers', async t => {
  t.mock.method(globalThis, 'fetch', async () => { throw new TypeError('offline') })
  assert.equal(await submitQuizResult(payload(), endpoint), 'failed')
})
test('share text includes score, branded link and no participant data; mastery is gated', () => {
  const achievement = { score: 14, total: 15, masteredTotal: 30, bankTotal: 50 }
  const text = badgeText('quiz', achievement, canonicalQuizUrl)
  assert.match(text, /14\/15 \(93%\)/)
  assert.match(text, /ace-cdl-explorer\/quiz.html/)
  assert.doesNotMatch(text, /learner@example|BACnet|certified/)
  assert.throws(() => badgeText('mastery', achievement, canonicalQuizUrl), /locked/)
  assert.match(badgeText('mastery', { ...achievement, masteredTotal: 50 }, canonicalQuizUrl), /50\/50/)
  assert.equal(quizShareUrl('./', 'https://example.com/cdl/?private=yes#quiz'), 'https://example.com/cdl/quiz.html')
})
test('quiz landing page has crawlable metadata and a 1200 by 627 PNG', () => {
  const html = readFileSync(new URL('../public/quiz.html', import.meta.url), 'utf8')
  assert.ok(html.includes(`property="og:url" content="${canonicalQuizUrl}"`))
  assert.match(html, /cdl-quiz-social.png/)
  assert.ok(html.includes("new URL('./#quiz', window.location.href)"))
  assert.doesNotMatch(html, /BACnet|35.question|100.question/)
  const png = readFileSync(new URL('../public/brand/cdl-quiz-social.png', import.meta.url))
  assert.equal(png.subarray(1, 4).toString('ascii'), 'PNG')
  assert.equal(png.readUInt32BE(16), 1200)
  assert.equal(png.readUInt32BE(20), 627)
})
function receiver() {
  const rows: unknown[][] = []
  const ctx: Record<string, any> = { console: { error() {} },
    ContentService: { MimeType: { JSON: 'json' }, createTextOutput: (text: string) => ({ text, setMimeType() { return this } }) },
    LockService: { getScriptLock: () => ({ waitLock() {}, releaseLock() {} }) },
    PropertiesService: { getScriptProperties: () => ({ getProperty: (key: string) => key === 'SPREADSHEET_ID' ? 'test-workbook' : null }) },
    SpreadsheetApp: { getActiveSpreadsheet: () => { throw new Error('Not available in web apps') }, openById: (id: string) => { assert.equal(id, 'test-workbook'); return { getSheetByName: () => sheet } } },
  }
  const sheet = { getLastRow: () => rows.length,
    appendRow: (row: unknown[]) => { rows.push(row) }, setFrozenRows() {},
    getRange(row: number, column: number) { return {
      setValues(value: unknown[][]) { if (row === 1) rows[0] = value[0]!; else value[0]!.forEach((v, i) => { rows[row - 1]![column - 1 + i] = v }) },
      setFontWeight() {}, createTextFinder: (id: string) => ({ matchEntireCell() { return this }, findNext: () => {
        const index = rows.findIndex(row => row[1] === id)
        return index >= 0 ? { getRow: () => index + 1 } : null
      } }),
    } },
  }
  runInNewContext(readFileSync(new URL('../../integrations/google-apps-script/Code.gs', import.meta.url), 'utf8'), ctx)
  const post = (value: unknown) => JSON.parse(ctx.doPost({ parameter: { payload: JSON.stringify(value) } }).text)
  return { ctx, rows, post }
}
test('Sheets receiver records consent, keeps one row per session and does not duplicate Pipedrive work', () => {
  const service = receiver(), result = payload()
  assert.equal(service.post(result).ok, true)
  assert.equal(service.rows.length, 2)
  assert.equal(service.rows[1]![2], result.email)
  assert.equal(service.rows[1]![15], 'not_configured')
  assert.equal(service.rows[1]![17], result.consentAt)
  assert.equal(service.post(result).duplicate, true)
  assert.equal(service.rows.length, 2)
})
test('Sheets persistence succeeds despite a Pipedrive failure', () => {
  const service = receiver()
  service.ctx.syncPipedrive_ = () => { throw new Error('Pipedrive unavailable') }
  assert.equal(service.post(payload()).ok, true)
  assert.equal(service.rows.length, 2)
  assert.equal(service.rows[1]![15], 'failed')
})
test('receiver rejects malformed, cross-app and spam payloads before storage', () => {
  const service = receiver(), result = payload()
  for (const bad of [{ ...result, appId: 'bacnet-studio' }, { ...result, total: 35 }, { ...result, questionIds: [...result.questionIds.slice(1), result.questionIds[1]] },
    { ...result, consentAt: '' }, { ...result, website: 'spam' }, { ...result, answers: {} }, { ...result, percentage: 0 }]) {
    assert.equal(service.post(bad).ok, false)
  }
  assert.equal(service.rows.length, 0)
})
test('receiver escapes formula-like email cells', () => {
  const service = receiver()
  assert.equal(service.post({ ...payload(), email: '+learner@example.com' }).ok, true)
  assert.equal(service.rows[1]![2], "'+learner@example.com")
})

test('Pipedrive matches exact email or creates a person, then adds a CDL result note', () => {
  for (const matched of [true, false]) {
    const service = receiver()
    const requests: { url: string; options: any }[] = []
    service.ctx.PropertiesService = { getScriptProperties: () => ({ getProperty: (key: string) => ({ PIPEDRIVE_API_TOKEN: 'test-only-token', PIPEDRIVE_COMPANY_DOMAIN: 'test-company', SPREADSHEET_ID: 'test-workbook' })[key] ?? null }) }
    service.ctx.UrlFetchApp = { fetch: (url: string, options: any) => {
      requests.push({ url, options })
      const data = url.includes('/search') ? { items: matched ? [{ item: { id: 42 } }] : [] } : { id: 42 }
      return { getResponseCode: () => 200, getContentText: () => JSON.stringify({ success: true, data }) }
    } }
    const result = payload()
    assert.equal(service.post(result).ok, true)
    assert.match(requests[0]!.url, /exact_match=true/)
    assert.equal(requests.length, matched ? 2 : 3)
    const note = JSON.parse(requests.at(-1)!.options.payload)
    assert.equal(note.person_id, 42)
    assert.match(note.content, /ACE CDL Explorer competency quiz/)
    assert.equal(service.rows[1]![15], matched ? 'matched_and_noted' : 'created_and_noted')
    assert.equal(service.post(result).duplicate, true)
    assert.equal(requests.length, matched ? 2 : 3)
  }
})

test('setup stores the workbook ID and web requests do not require an active spreadsheet', () => {
  const service = receiver()
  const values = new Map<string, string>()
  const properties = { getProperty: (key: string) => values.get(key) ?? null,
    setProperty: (key: string, value: string) => { values.set(key, value); return properties } }
  service.ctx.PropertiesService = { getScriptProperties: () => properties }
  const workbook = service.ctx.SpreadsheetApp.openById('test-workbook')
  service.ctx.SpreadsheetApp.getActiveSpreadsheet = () => ({ ...workbook, getId: () => 'test-workbook', getName: () => 'Test workbook' })
  assert.match(service.ctx.setupQuizResults(), /Test workbook/)
  assert.equal(values.get('SPREADSHEET_ID'), 'test-workbook')
  service.ctx.SpreadsheetApp.getActiveSpreadsheet = () => { throw new Error('No active file in web app') }
  assert.equal(service.post(payload()).ok, true)
})
