import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const QWEN_MODEL_KEY = 'qwen/qwen2.5-0.5b-instruct';

test('app opens into the Chat product center', async ({ page }) => {
  await page.goto('/');

  await expect(page).toHaveURL(/\/chat$/);
  await expect(page.getByRole('link', { name: 'Lattice chat' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Chat' })).toBeVisible();
  await expect(page.getByText('Local runtime is not configured.')).toBeVisible();
  await expect(page.getByText('Configure a local runtime first.')).toBeVisible();
});

test('primary navigation moves between Chat, Models, and Settings', async ({ page }) => {
  await page.goto('/');

  await page.getByRole('link', { name: 'Models', exact: true }).click();
  await expect(page).toHaveURL(/\/models$/);
  await expect(page.getByRole('heading', { name: 'Models', exact: true })).toBeVisible();

  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await expect(page).toHaveURL(/\/settings$/);
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

  await page.getByRole('link', { name: 'Chat', exact: true }).click();
  await expect(page).toHaveURL(/\/chat$/);
});

test('Chat shows a no-model state after the simulated runtime starts', async ({ page }) => {
  await configureAndStartRuntime(page);
  await page.getByRole('link', { name: 'Chat', exact: true }).click();

  await expect(page.getByText('Choose a local model to start chatting.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Choose model' })).toBeVisible();
});

test('loading a simulated model makes Chat ready', async ({ page }) => {
  await loadQwenModel(page);
  await page.getByRole('link', { name: 'Chat', exact: true }).click();

  await expect(page.getByText(QWEN_MODEL_KEY, { exact: true })).toBeVisible();
  await expect(page.getByText('Ready when you are.')).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Message' })).toBeEnabled();
});

test('a chat response streams and appears in Chat history', async ({ page }) => {
  await loadQwenModel(page);
  await page.getByRole('link', { name: 'Chat', exact: true }).click();

  await sendMessage(page, 'Hello there');

  await expect(page.locator('.message.assistant .text')).toContainText('simulated', {
    timeout: 10_000
  });
  await expect(page.getByRole('textbox', { name: 'Message' })).toBeEnabled({ timeout: 10_000 });
  await expect(page.locator('.message.assistant .text')).toHaveText(
    'This is a simulated response because Lattice is running as a browser preview without the desktop shell.'
  );
  await expect(
    page.locator('.conversation-button').filter({ hasText: 'Hello there' })
  ).toBeVisible();
});

test('a chat response can be stopped mid-stream', async ({ page }) => {
  await loadQwenModel(page);
  await page.getByRole('link', { name: 'Chat', exact: true }).click();

  await sendMessage(page, 'Please stop soon');
  await expect(page.getByRole('button', { name: 'Stop', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Stop', exact: true }).click();

  await expect(page.locator('.message.assistant .status-tag')).toHaveText('Cancelled');
  await expect(page.getByRole('textbox', { name: 'Message' })).toBeEnabled();
});

test('a conversation can be reopened, replaced by a new chat, and deleted from full history', async ({
  page
}) => {
  await loadQwenModel(page);
  await page.getByRole('link', { name: 'Chat', exact: true }).click();
  await sendMessage(page, 'Remember this local chat');
  await expect(page.getByRole('textbox', { name: 'Message' })).toBeEnabled({ timeout: 10_000 });

  await page
    .locator('.conversation-button')
    .filter({ hasText: 'Remember this local chat' })
    .click();
  await expect(page).toHaveURL(/\/chat\?conversationId=/);
  await expect(page.locator('.message.assistant .text')).toContainText('simulated');

  await page.getByRole('button', { name: 'New chat' }).click();
  await expect(page.getByText('Ready when you are.')).toBeVisible();

  await page.getByRole('link', { name: 'Full history' }).click();
  await expect(page).toHaveURL(/\/history$/);
  await expect(page.getByRole('heading', { name: 'History' })).toBeVisible();
  await expect(page.getByText('Remember this local chat')).toBeVisible();

  await page.getByRole('button', { name: 'Delete' }).click();
  await page.getByRole('button', { name: 'Yes' }).click();
  await expect(page.getByText('No conversations yet.')).toBeVisible();
});

test('settings can be edited and advanced diagnostics are accessible', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Settings', exact: true }).click();

  await expect(page.getByRole('heading', { name: 'General' })).toBeVisible();
  await page.getByRole('button', { name: 'Dark' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-appearance', 'dark');
  await expect(page.getByText('Selected: Dark')).toBeVisible();
  await page.getByRole('link', { name: 'Chat', exact: true }).click();
  await expect(page.locator('html')).not.toHaveAttribute('data-appearance', 'dark');

  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Light' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-appearance', 'light');
  await page.getByRole('button', { name: 'Dark' }).click();
  await page.getByRole('spinbutton', { name: 'Minutes' }).fill('12');
  await page.getByRole('button', { name: 'Save' }).click();

  await expect(page.getByText('Revision 2 · Schema 2')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-appearance', 'dark');
  await expect(page.getByRole('heading', { name: 'Advanced' })).toBeVisible();
  await expect(page.getByText('Application')).toBeVisible();
  await expect(page.getByText('web · debug')).toBeVisible();
});

test('a credential can be added, replaced, and removed in browser preview', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Settings', exact: true }).click();

  await expect(page.getByRole('heading', { name: 'Credentials', exact: true })).toBeVisible();
  await expect(page.getByText('No credentials yet.')).toBeVisible();

  await page.getByRole('textbox', { name: 'Label' }).fill('Personal provider key');
  await page.getByRole('textbox', { name: 'Credential key' }).fill('local-preview');
  await page.getByRole('button', { name: 'Add' }).click();

  const credentialCard = page.locator('.credential-card');
  await expect(credentialCard.getByText('Personal provider key', { exact: true })).toBeVisible();
  await expect(credentialCard.getByText('Stored securely · local-preview')).toBeVisible();

  await credentialCard.getByRole('button', { name: 'Replace' }).click();
  await expect(credentialCard.getByText('Stored securely · local-preview')).toBeVisible();

  await credentialCard.getByRole('button', { name: 'Remove' }).click();
  await credentialCard.getByRole('button', { name: 'Yes' }).click();
  await expect(page.getByText('No credentials yet.')).toBeVisible();
});

async function configureAndStartRuntime(page: Page): Promise<void> {
  await page.goto('/');
  await page.getByRole('link', { name: 'Models', exact: true }).click();
  await page.getByRole('textbox', { name: 'Path' }).fill('/usr/local/bin/lms');
  await page.getByRole('button', { name: 'Configure' }).click();
  await page.getByRole('button', { name: 'Probe' }).click();
  await expect(page.getByText('Stopped', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Start' }).click();
  await expect(page.getByText('Running', { exact: true })).toBeVisible();
}

async function loadQwenModel(page: Page): Promise<void> {
  await configureAndStartRuntime(page);

  const loadButton = page.getByRole('listitem').filter({ hasText: 'Qwen2.5' }).getByRole('button');
  await expect(loadButton).toBeEnabled();
  await loadButton.click();

  await expect(page.getByText(QWEN_MODEL_KEY, { exact: true })).toBeVisible();
}

async function sendMessage(page: Page, message: string): Promise<void> {
  const input = page.getByRole('textbox', { name: 'Message' });
  await input.fill(message);
  await page.getByRole('button', { name: 'Send' }).click();
}
