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

  await expect(page.getByText('Revision 2 · Schema 2')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-appearance', 'dark');
});

test('model runtime discovery can be configured in browser smoke mode', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Models', exact: true }).click();

  await expect(page).toHaveURL(/\/models$/);
  await expect(page.getByRole('heading', { name: 'Runtime discovery' })).toBeVisible();
  await page.getByRole('textbox', { name: 'Path' }).fill('/usr/local/bin/lms');
  await page.getByRole('button', { name: 'Configure' }).click();
  await expect(page.getByText('Approve a runtime probe')).toBeVisible();
  await page.getByRole('button', { name: 'Probe' }).click();

  await expect(page.getByText('stopped').first()).toBeVisible();
  await expect(page.getByText('0.0.47')).toBeVisible();
});

test('model runtime lifecycle can be started and stopped in browser smoke mode', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Models', exact: true }).click();

  await expect(page).toHaveURL(/\/models$/);
  await page.getByRole('textbox', { name: 'Path' }).fill('/usr/local/bin/lms');
  await page.getByRole('button', { name: 'Configure' }).click();
  await page.getByRole('button', { name: 'Probe' }).click();
  await expect(page.getByText('stopped').first()).toBeVisible();

  const startButton = page.getByRole('button', { name: 'Start' });
  await expect(startButton).toBeEnabled();
  await startButton.click();

  await expect(page.getByText('running').first()).toBeVisible();
  await expect(page.getByText('owned')).toBeVisible();
  await expect(page.getByText('Last operation: started')).toBeVisible();

  const stopButton = page.getByRole('button', { name: 'Stop' });
  await expect(stopButton).toBeEnabled();
  await stopButton.click();

  await expect(page.getByText('stopped').first()).toBeVisible();
  await expect(page.getByText('unknown').first()).toBeVisible();
  await expect(page.getByText('Last operation: stopped')).toBeVisible();
});
