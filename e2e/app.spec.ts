import { expect, test, type Page } from '@playwright/test';

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

test('installed models can be loaded and unloaded in browser smoke mode', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Models', exact: true }).click();

  await expect(page).toHaveURL(/\/models$/);
  await page.getByRole('textbox', { name: 'Path' }).fill('/usr/local/bin/lms');
  await page.getByRole('button', { name: 'Configure' }).click();
  await page.getByRole('button', { name: 'Probe' }).click();
  await expect(page.getByText('stopped').first()).toBeVisible();
  await page.getByRole('button', { name: 'Start' }).click();
  await expect(page.getByText('running').first()).toBeVisible();

  await expect(page.getByRole('heading', { name: 'Installed models' })).toBeVisible();
  const loadButton = page.getByRole('listitem').filter({ hasText: 'Qwen2.5' }).getByRole('button');
  await expect(loadButton).toBeEnabled();
  await loadButton.click();

  const modelCard = page.locator('.model-card');
  await expect(modelCard.getByText('qwen/qwen2.5-0.5b-instruct')).toBeVisible();
  await expect(modelCard.getByText('owned')).toBeVisible();
  await expect(modelCard.getByText('Last operation: loaded')).toBeVisible();

  const unloadButton = page.getByRole('button', { name: 'Unload' });
  await expect(unloadButton).toBeEnabled();
  await unloadButton.click();

  await expect(modelCard.getByText('None')).toBeVisible();
  await expect(modelCard.getByText('Last operation: unloaded')).toBeVisible();
});

async function loadQwenModel(page: Page): Promise<void> {
  await page.goto('/');
  await page.getByRole('link', { name: 'Models', exact: true }).click();
  await page.getByRole('textbox', { name: 'Path' }).fill('/usr/local/bin/lms');
  await page.getByRole('button', { name: 'Configure' }).click();
  await page.getByRole('button', { name: 'Probe' }).click();
  await expect(page.getByText('stopped').first()).toBeVisible();
  await page.getByRole('button', { name: 'Start' }).click();
  await expect(page.getByText('running').first()).toBeVisible();

  const loadButton = page.getByRole('listitem').filter({ hasText: 'Qwen2.5' }).getByRole('button');
  await expect(loadButton).toBeEnabled();
  await loadButton.click();
  await expect(page.locator('.model-card').getByText('owned')).toBeVisible();
}

test('a chat response streams incrementally and completes in browser smoke mode', async ({
  page
}) => {
  await loadQwenModel(page);
  await page.getByRole('link', { name: 'Chat', exact: true }).click();

  await expect(page).toHaveURL(/\/chat$/);
  await expect(page.getByRole('heading', { name: 'Chat' })).toBeVisible();
  await expect(page.getByText('qwen/qwen2.5-0.5b-instruct')).toBeVisible();

  const input = page.getByRole('textbox', { name: 'Message' });
  await input.fill('Hello there');
  await page.getByRole('button', { name: 'Send' }).click();

  await expect(page.locator('.message.assistant .text')).toContainText('simulated', {
    timeout: 10_000
  });
  await expect(page.getByRole('button', { name: 'Send' })).toBeEnabled({ timeout: 10_000 });
  await expect(page.locator('.message.assistant .text')).toHaveText(
    'This is a simulated response because Lattice is running as a browser preview without the desktop shell.'
  );
});

test('a chat response can be cancelled mid-stream in browser smoke mode', async ({ page }) => {
  await loadQwenModel(page);
  await page.getByRole('link', { name: 'Chat', exact: true }).click();

  const input = page.getByRole('textbox', { name: 'Message' });
  await input.fill('Hello there');
  await page.getByRole('button', { name: 'Send' }).click();

  await expect(page.getByRole('button', { name: 'Cancel' })).toBeVisible();
  await page.getByRole('button', { name: 'Cancel' }).click();

  await expect(page.locator('.message.assistant .status-tag')).toHaveText('Cancelled');
  await expect(page.getByRole('button', { name: 'Send' })).toBeEnabled();
});
