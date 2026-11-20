import { expect, test } from '@playwright/test'

test('Buildings symbols retain typed identity across the glossary and palette', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto('/#glossary')
  await page.getByRole('button', { name: 'Glossary', exact: true }).click()
  const upstream = page.locator('.block-glossary-card .block-symbol[data-symbol-kind=buildings]')
  await expect(upstream).toHaveCount(138) // 132 standard classes and six legacy aliases
  for (const family of ['Reals', 'Integers', 'Logical']) {
    await expect(page.locator(`.block-glossary-card svg[data-symbol-source="Buildings.Controls.OBC.CDL.${family}.Switch"]`)).toHaveCount(family === 'Reals' ? 2 : 1)
  }
  await page.getByRole('button', { name: 'Visual editor', exact: true }).click()
  const add = page.locator('.palette-item').filter({ hasText: 'Weighted add' })
  await expect(add.locator('svg')).toHaveAttribute('data-symbol-source', 'Buildings.Controls.OBC.CDL.Reals.Add')
  await add.click()
  await expect(page.locator('.graph-node.selected .block-symbol')).toHaveAttribute('data-symbol-source', 'Buildings.Controls.OBC.CDL.Reals.Add')
  await expect(page.getByLabel('k1', { exact: true })).toHaveValue('1')
  await expect(page.locator('.palette-item').filter({ hasText: 'VAV template' }).locator('svg')).toHaveAttribute('data-symbol-kind', 'ace')
  expect(errors).toEqual([])
})

test('primer graphs share the source symbols and wires meet their connectors', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Primer', exact: true }).click()
  const hero = page.locator('.primer-graph-hero')
  await expect(hero.locator('svg[data-symbol-source="Buildings.Controls.OBC.CDL.Reals.Sources.Constant"]')).toBeVisible()
  await expect(hero.locator('svg[data-symbol-source="Buildings.Controls.OBC.CDL.Reals.Subtract"]')).toBeVisible()
  await expect(hero.locator('.edge-layer path')).toHaveCount(2)
  for (const [index, port] of ['u1', 'u2'].entries()) {
    const dot = await hero.locator(`[data-anchor="cool_error.${port}"]`).boundingBox()
    const wire = await hero.locator('.edge-layer circle').nth(index).boundingBox()
    expect(dot).not.toBeNull()
    expect(wire).not.toBeNull()
    expect(Math.abs((dot!.x + dot!.width / 2) - (wire!.x + wire!.width / 2))).toBeLessThan(1)
    expect(Math.abs((dot!.y + dot!.height / 2) - (wire!.y + wire!.height / 2))).toBeLessThan(1)
  }
})
