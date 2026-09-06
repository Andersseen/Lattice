import { expect, test } from '@playwright/test';

test('application loads the foundation shell', async ({ page }) => {
  await page.goto('/');

  await expect(page.getByRole('link', { name: 'Lattice home' })).toBeVisible();
  await expect(page.getByRole('heading', { name: /Local-first agentic workspace/ })).toBeVisible();
  await expect(page.getByText('Angular zoneless')).toBeVisible();
  await expect(page.getByText('web / debug')).toBeVisible();
});

test('basic navigation works', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'System', exact: true }).click();

  await expect(page).toHaveURL(/\/system$/);
  await expect(page.getByRole('heading', { name: /typed bridge/ })).toBeVisible();
});
