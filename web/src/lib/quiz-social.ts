export type BadgeKind = 'quiz' | 'mastery'
export interface QuizAchievement { score: number; total: number; masteredTotal: number; bankTotal: number }
export const canonicalQuizUrl = 'https://ace-iot-solutions.github.io/ace-cdl-explorer/quiz.html'
export function quizShareUrl(baseUrl: string, location: string): string {
  const directory = new URL(baseUrl, location)
  return new URL('quiz.html', directory).toString()
}
export function badgeText(kind: BadgeKind, result: QuizAchievement, url: string): string {
  if (kind === 'mastery' && result.masteredTotal < result.bankTotal) throw new Error('Mastery badge is locked.')
  return kind === 'mastery' ? [
    `🏆 ACE CDL Explorer · ${result.bankTotal}/${result.bankTotal} Question Bank Mastery`,
    `I correctly answered all ${result.bankTotal} questions in the CDL competency practice bank.`,
    `Practice Standard 231 concepts: ${url}`, '#CDL #ASHRAE231 #BuildingAutomation',
  ].join('\n') : [
    `🏅 ACE CDL Explorer · Quiz Complete · ${result.score}/${result.total} (${Math.round(result.score / result.total * 100)}%)`,
    'I completed the CDL competency practice quiz. Can you beat my score?',
    `Take the quiz: ${url}`, '#CDL #ASHRAE231 #BuildingAutomation',
  ].join('\n')
}
/** Only public score counts enter rendering: no participant email or answer history. */
export function drawBadgeImage(kind: BadgeKind, result: QuizAchievement): HTMLCanvasElement {
  if (kind === 'mastery' && result.masteredTotal < result.bankTotal) throw new Error('Mastery badge is locked.')
  const canvas = document.createElement('canvas')
  canvas.width = 1200; canvas.height = 627
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('Canvas unavailable')
  const background = ctx.createLinearGradient(0, 0, 1200, 627)
  background.addColorStop(0, '#11131a'); background.addColorStop(1, '#292e3d')
  ctx.fillStyle = background; ctx.fillRect(0, 0, 1200, 627)
  ctx.fillStyle = kind === 'mastery' ? '#f5be3f' : '#c1d301'
  ctx.fillRect(64, 58, 72, 6)
  ctx.font = '700 23px Arial, sans-serif'
  ctx.fillText('ACE CDL EXPLORER', 64, 109)
  ctx.fillStyle = '#fff'; ctx.font = '800 108px Arial, sans-serif'
  ctx.fillText(kind === 'mastery' ? `${result.bankTotal} / ${result.bankTotal}` : `${Math.round(result.score / result.total * 100)}%`, 60, 252)
  ctx.font = '700 45px Arial, sans-serif'
  ctx.fillText(kind === 'mastery' ? 'Question bank mastered' : 'Quiz complete', 64, 326)
  ctx.fillStyle = '#cbd0dd'; ctx.font = '400 28px Arial, sans-serif'
  ctx.fillText(kind === 'mastery' ? 'Every bank question answered correctly' : `${result.score} of ${result.total} questions answered correctly`, 64, 382)
  ctx.fillStyle = '#a5adbf'; ctx.font = '400 20px Arial, sans-serif'
  ctx.fillText('CDL · Abstract signals · Connections · Block semantics', 64, 472)
  ctx.fillText('Practice achievement · not a certification', 64, 514)
  ctx.font = '700 19px Arial, sans-serif'; ctx.fillStyle = '#c1d301'
  ctx.fillText('ace-iot-solutions.github.io/ace-cdl-explorer/quiz.html', 64, 569)
  ctx.strokeStyle = '#42c8c1'; ctx.lineWidth = 4
  ctx.strokeRect(880, 90, 230, 100)
  ctx.fillStyle = '#fff'; ctx.font = '700 24px Arial, sans-serif'; ctx.fillText('CDL', 969, 149)
  ctx.beginPath(); ctx.moveTo(820, 140); ctx.lineTo(880, 140); ctx.moveTo(1110, 140); ctx.lineTo(1160, 140); ctx.stroke()
  ctx.fillStyle = '#c1d301'; ctx.font = '700 38px Arial, sans-serif'; ctx.fillText(kind === 'mastery' ? '★' : '✓', 1060, 250)
  return canvas
}
export async function badgeImageBlob(kind: BadgeKind, result: QuizAchievement): Promise<Blob> {
  // This fixed-size image is small enough to encode synchronously. Canvas.toBlob
  // can defer its idle callback indefinitely on repeated copies in Chromium.
  const encoded = drawBadgeImage(kind, result).toDataURL('image/png').split(',')[1]
  if (!encoded) throw new Error('PNG encoding failed')
  const bytes = Uint8Array.from(atob(encoded), character => character.charCodeAt(0))
  return new Blob([bytes], { type: 'image/png' })
}

export async function copyPostText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) { await navigator.clipboard.writeText(text); return }
  const textarea = document.createElement('textarea')
  textarea.value = text; textarea.readOnly = true
  textarea.style.position = 'fixed'; textarea.style.opacity = '0'
  document.body.append(textarea); textarea.select()
  try { if (!document.execCommand('copy')) throw new Error('Clipboard unavailable') } finally { textarea.remove() }
}
export function downloadBadge(blob: Blob, kind: BadgeKind, result: QuizAchievement): void {
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = kind === 'mastery' ? `ace-cdl-${result.bankTotal}-question-mastery.png` : `ace-cdl-quiz-${result.score}-of-${result.total}.png`
  document.body.append(link); link.click(); link.remove()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
