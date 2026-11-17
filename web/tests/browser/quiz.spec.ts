import { expect, test, type Page } from '@playwright/test'
import { readFileSync } from 'node:fs'
import type { QuizProgress, QuizQuestion } from '../../src/lib/quiz'
const questions = JSON.parse(readFileSync(new URL('../../src/data/cdlQuizQuestions.json', import.meta.url), 'utf8')) as QuizQuestion[]
const key = 'ace-cdl-quiz-v1'
// Existing quiz scenarios have an already-consented local participant.
test.beforeEach(async ({ page }, info) => {
  if (info.title.includes('email and consent')) return
  await page.addInitScript(() => {
    localStorage.setItem('ace-cdl-quiz-participant-v1', JSON.stringify({ version: 1, email: 'learner@example.com', consentAt: '2026-11-16T12:00:00.000Z' }))
  })
})

async function saved(page: Page) {
  return page.evaluate(key => JSON.parse(localStorage.getItem(key)!) as QuizProgress, key)
}
async function checkCurrent(page: Page) {
  const progress = await saved(page)
  const id = progress.session!.questionIds[progress.session!.index]
  const q = questions.find(q => q.id === id)!
  await page.locator(`input[type=radio][value="${q.correctOptionId}"]`).check()
  await page.getByRole('button', { name: 'Check answer', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Correct.', exact: true })).toBeVisible()
}

test('Quiz deep link, checked-answer reload and tab navigation preserve the active round', async ({ page }) => {
  await page.goto('/#quiz')
  await expect(page.getByRole('heading', { name: 'Put your CDL knowledge to work.' })).toBeVisible()
  await page.getByRole('button', { name: 'Start 15-question quiz' }).click()
  await expect(page.getByRole('button', { name: 'Check answer' })).toBeDisabled()
  const initial = await saved(page)
  expect(new Set(initial.session!.questionIds).size).toBe(15)
  await checkCurrent(page)
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Correct.', exact: true })).toBeVisible()
  expect((await saved(page)).session!.questionIds).toEqual(initial.session!.questionIds)
  await page.getByRole('button', { name: 'Next question' }).click()
  await page.getByRole('button', { name: 'Glossary', exact: true }).click()
  await page.getByRole('button', { name: 'Quiz', exact: true }).click()
  await expect(page.getByText('Question 2 of 15', { exact: true })).toBeVisible()
  expect((await saved(page)).session!.index).toBe(1)
})

test('completing fifteen correct answers excludes them from the next round', async ({ page }) => {
  await page.goto('/#quiz')
  await page.getByRole('button', { name: 'Start 15-question quiz' }).click()
  const first = (await saved(page)).session!.questionIds
  for (let index = 0; index < 15; index++) {
    await checkCurrent(page)
    await page.getByRole('button', { name: index === 14 ? 'See results' : 'Next question', exact: true }).click()
  }
  await expect(page.getByRole('heading', { name: '15 of 15 correct.' })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('heading', { name: '15 of 15 correct.' })).toBeVisible()
  await expect(page.getByTestId('copy-mastery-badge')).toHaveCount(0)
  await page.getByRole('button', { name: 'Start another 15-question quiz' }).click()
  expect((await saved(page)).session!.questionIds.every(id => !first.includes(id))).toBe(true)
})

test('exhaustion returns review questions and reset affects only quiz progress', async ({ page }) => {
  const history = Object.fromEntries(questions.slice(0, 48).map(q => [q.id, [{ choiceId: q.correctOptionId, revision: q.revision, answeredAt: '2026-11-16T12:00:00.000Z' }]]))
  await page.addInitScript(({ key, history }) => {
    localStorage.setItem(key, JSON.stringify({ version: 1, history, session: null }))
    localStorage.setItem('quiz-unrelated-sentinel', 'keep')
  }, { key, history })
  await page.goto('/#quiz')
  await page.getByRole('button', { name: 'Start 15-question quiz' }).click()
  const ids = (await saved(page)).session!.questionIds
  expect(new Set(ids.slice(0, 2))).toEqual(new Set(questions.slice(48).map(q => q.id)))
  expect(ids).toHaveLength(15)
  await page.getByRole('button', { name: 'Reset quiz progress', exact: true }).click()
  await page.getByRole('button', { name: 'Keep progress', exact: true }).click()
  expect((await saved(page)).session).not.toBeNull()
  await page.getByRole('button', { name: 'Reset quiz progress', exact: true }).click()
  await page.getByRole('button', { name: 'Clear quiz progress', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Start 15-question quiz' })).toBeVisible()
  expect((await saved(page)).history).toEqual({})
  expect(await page.evaluate(() => localStorage.getItem('quiz-unrelated-sentinel'))).toBe('keep')
})

test('unavailable quiz storage shows a warning and still permits answering on mobile', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.addInitScript(key => {
    const original = Storage.prototype.setItem
    Storage.prototype.setItem = function (name, value) {
      if (name === key) throw new Error('Storage unavailable')
      return original.call(this, name, value)
    }
  }, key)
  await page.goto('/#quiz')
  await page.getByRole('button', { name: 'Start 15-question quiz' }).click()
  await expect(page.getByText('Progress could not be saved on this browser.', { exact: false })).toBeVisible()
  await page.getByRole('radio').first().check()
  await page.getByRole('button', { name: 'Check answer', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Next question', exact: true })).toBeVisible()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

test('email and consent are required before starting a quiz', async ({ page }) => {
  await page.goto('/#quiz')
  await expect(page.getByRole('heading', { name: 'Enter your email to begin' })).toBeVisible()
  await page.getByLabel('Email address').fill('learner@example.com')
  await page.getByRole('button', { name: 'Start competency quiz' }).click()
  await expect(page.getByRole('radio')).toHaveCount(0)
  await page.getByRole('checkbox').check()
  await page.getByRole('button', { name: 'Start competency quiz' }).click()
  await expect(page.getByText('Question 1 of 15', { exact: true })).toBeVisible()
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('ace-cdl-quiz-participant-v1')!).email)).toBe('learner@example.com')
  expect((await saved(page)).session!.participantEmail).toBe('learner@example.com')
})

test('completed quiz shares branded text and PNG badges without email or live submissions', async ({ page }) => {
  const ids = questions.slice(0, 15).map(q => q.id)
  const history = Object.fromEntries(questions.map(q => [q.id, [{ choiceId: q.correctOptionId, revision: q.revision, answeredAt: '2026-11-16T12:00:00.000Z' }]]))
  const round = { sessionId: 'completed-quiz-round', startedAt: '2026-11-16T12:00:00.000Z', completedAt: '2026-11-16T12:10:00.000Z', participantEmail: 'learner@example.com',
    questionIds: ids, revisions: ids.map(() => 2), answers: Object.fromEntries(questions.slice(0, 15).map(q => [q.id, q.correctOptionId])), index: 15 }
  await page.addInitScript(({ key, history, round }) => {
    localStorage.setItem(key, JSON.stringify({ version: 1, history, session: round }))
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
      writeText: async (text: string) => { (window as any).__copiedPost = text },
      write: async (items: ClipboardItem[]) => { const blob = await items[0]!.getType('image/png'); (window as any).__copiedImage = { type: blob.type, size: blob.size }; (window as any).__imageCopies = ((window as any).__imageCopies ?? 0) + 1 },
    } })
  }, { key, history, round })
  const submissions: string[] = []
  page.on('request', request => { if (request.url().includes('script.google.com')) submissions.push(request.url()) })
  await page.goto('/#quiz')
  await expect(page.getByRole('heading', { name: '15 of 15 correct.' })).toBeVisible()
  await expect(page.getByText('Results are saved in this browser. Remote collection is not connected here.')).toBeVisible()
  await page.getByTestId('copy-quiz-badge').click()
  const post = await page.evaluate(() => (window as any).__copiedPost)
  expect(post).toContain('15/15 (100%)')
  expect(post).toContain('quiz.html')
  expect(post).not.toContain('learner@example.com')
  await page.getByTestId('copy-quiz-image').click()
  await expect(page.getByText('Badge image copied. Paste it into your social post.')).toBeVisible()
  expect((await page.evaluate(() => (window as any).__copiedImage)).size).toBeGreaterThan(1000)
  for (let copies = 2; copies <= 3; copies++) {
    await page.getByTestId('copy-quiz-image').click()
    await expect.poll(() => page.evaluate(() => (window as any).__imageCopies)).toBe(copies)
  }
  const [download] = await Promise.all([page.waitForEvent('download'), page.getByTestId('download-quiz-image').click()])
  expect(download.suggestedFilename()).toBe('ace-cdl-quiz-15-of-15.png')
  const image = readFileSync((await download.path())!)
  expect(image.readUInt32BE(16)).toBe(1200)
  expect(image.readUInt32BE(20)).toBe(627)
  await page.getByTestId('copy-mastery-badge').click()
  expect(await page.evaluate(() => (window as any).__copiedPost)).toContain('50/50')
  expect(submissions).toEqual([])
})

test('social landing URL redirects to the quiz', async ({ page }) => {
  await page.goto('/quiz.html')
  await expect(page).toHaveURL(/\/#quiz$/)
  await expect(page.getByRole('heading', { name: 'Put your CDL knowledge to work.' })).toBeVisible()
})
