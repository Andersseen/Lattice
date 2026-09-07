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

test('settings can be edited in browser smoke mode', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Settings', exact: true }).click();

  await expect(page).toHaveURL(/\/settings$/);
  await expect(page.getByRole('heading', { name: 'Preferences' })).toBeVisible();
  await page.getByRole('button', { name: 'Dark' }).click();
  await page.getByRole('spinbutton', { name: 'Minutes' }).fill('12');
  await page.getByRole('button', { name: 'Save' }).click();

  await expect(page.getByText('Revision 2 · Schema 1')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-appearance', 'dark');
});
