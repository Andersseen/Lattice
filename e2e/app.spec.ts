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

  await expect(page.locator('.message.assistant').getByText('Cancelled')).toBeVisible();
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
  await page.getByRole('dialog').getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(page.getByText('No conversations yet.')).toBeVisible();
});

test('settings can be edited and advanced diagnostics are accessible', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Settings', exact: true }).click();

  await expect(page.getByRole('heading', { name: 'General' })).toBeVisible();
  await page.getByRole('tab', { name: 'Dark' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-appearance', 'dark');
  await expect(page.getByText('Selected: Dark')).toBeVisible();
  await page.getByRole('link', { name: 'Chat', exact: true }).click();
  await expect(page.locator('html')).not.toHaveAttribute('data-appearance', 'dark');

  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Light' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-appearance', 'light');
  await page.getByRole('tab', { name: 'Dark' }).click();
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
  await page.getByRole('button', { name: 'Add', exact: true }).click();

  const credentialCard = page.locator('.credential-card');
  await expect(credentialCard.getByText('Personal provider key', { exact: true })).toBeVisible();
  await expect(credentialCard.getByText('Stored securely')).toBeVisible();
  await expect(credentialCard.getByText('local-preview')).toBeVisible();

  await credentialCard.getByRole('button', { name: 'Replace' }).click();
  await expect(credentialCard.getByText('Stored securely')).toBeVisible();
  await expect(credentialCard.getByText('local-preview')).toBeVisible();

  await credentialCard.getByRole('button', { name: 'Remove' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(page.getByText('No credentials yet.')).toBeVisible();
});

test('a remote provider can be added, edited, rebound, and removed in Settings', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await addCredential(page, 'Remote key');
  await expect(page.getByText('No remote providers yet.')).toBeVisible();

  await page.getByRole('textbox', { name: 'Provider name' }).fill('Example AI');
  await page
    .getByRole('textbox', { name: 'Endpoint', exact: true })
    .fill('http://api.example.com/v1');
  await page.getByRole('textbox', { name: 'Model name', exact: true }).fill('gpt-test');
  await page.getByRole('button', { name: 'Add provider' }).click();
  await expect(page.getByText('Remote endpoints must start with https://.')).toBeVisible();

  await page
    .getByRole('textbox', { name: 'Endpoint', exact: true })
    .fill('https://API.example.com/v1/');
  await page.getByRole('combobox', { name: 'Credential', exact: true }).selectOption({
    label: 'Remote key'
  });
  await page.getByRole('button', { name: 'Add provider' }).click();

  const providerCard = page.locator('.provider-card');
  const credentialSelect = providerCard.getByRole('combobox', {
    name: 'Credential for Example AI'
  });
  await expect(providerCard.getByText('https://api.example.com/v1 · gpt-test')).toBeVisible();
  await expect(providerCard.getByText('Not approved')).toBeVisible();
  await expect(credentialSelect.locator('option:checked')).toHaveText('Remote key');

  await providerCard.getByRole('button', { name: 'Edit' }).click();
  await providerCard
    .getByRole('textbox', { name: 'Edit endpoint' })
    .fill('https://other.example.com/v1');
  await expect(
    providerCard.getByText('Saving a new endpoint removes the credential binding and the approval.')
  ).toBeVisible();
  await providerCard.getByRole('button', { name: 'Save' }).click();
  await expect(providerCard.getByText('https://other.example.com/v1 · gpt-test')).toBeVisible();
  await expect(credentialSelect.locator('option:checked')).toHaveText('No credential');

  await credentialSelect.selectOption({ label: 'Remote key' });
  await expect(credentialSelect.locator('option:checked')).toHaveText('Remote key');

  await providerCard.getByRole('button', { name: 'Remove' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(page.getByText('No remote providers yet.')).toBeVisible();
});

test('one conversation switches local, remote after approval, then local and keeps provenance', async ({
  page
}) => {
  await loadQwenModel(page);
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await addCredential(page, 'Remote key');
  await addRemoteProvider(page, 'Example AI', 'https://api.example.com/v1', 'gpt-test');

  await page.getByRole('link', { name: 'Chat', exact: true }).click();
  await sendMessage(page, 'Local question');
  await expectReplyComplete(page, 1, LOCAL_SIMULATED_REPLY);

  const provider = page.getByRole('combobox', { name: 'Provider' });
  await provider.selectOption({ label: 'Example AI' });
  await expect(page.getByText('gpt-test', { exact: true })).toBeVisible();

  await sendMessage(page, 'Remote question');
  const disclosure = page.getByRole('dialog', { name: 'Send this conversation to Example AI?' });
  await expect(disclosure).toBeVisible();
  await expect(disclosure.getByText('https://api.example.com/v1')).toBeVisible();
  await expect(disclosure.getByText('Remote key')).toBeVisible();
  await disclosure.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.locator('.message')).toHaveCount(2);
  await expect(page.getByRole('textbox', { name: 'Message' })).toHaveValue('Remote question');

  await page.getByRole('button', { name: 'Send' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Allow and send' }).click();
  await expectReplyComplete(
    page,
    3,
    'This is a simulated remote response from Example AI because Lattice is running as a browser preview without the desktop shell.'
  );
  await expect(page.locator('.message.assistant .provenance').last()).toHaveText(
    '· Remote · gpt-test'
  );

  await provider.selectOption({ label: 'Local model' });
  await sendMessage(page, 'Back to local');
  await expectReplyComplete(page, 5, LOCAL_SIMULATED_REPLY);

  await page.getByRole('button', { name: 'New chat' }).click();
  await page.locator('.conversation-button').filter({ hasText: 'Local question' }).click();
  await expect(page.locator('.message')).toHaveCount(6);
  await expect(page.locator('.message.assistant .provenance')).toHaveText([
    `· Local · ${QWEN_MODEL_KEY}`,
    '· Remote · gpt-test',
    `· Local · ${QWEN_MODEL_KEY}`
  ]);

  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  const providerCard = page.locator('.provider-card');
  await expect(providerCard.getByText('Approved for this endpoint')).toBeVisible();
  await providerCard.getByRole('button', { name: 'Revoke approval' }).click();
  await expect(providerCard.getByText('Not approved')).toBeVisible();
});

const LOCAL_SIMULATED_REPLY =
  'This is a simulated response because Lattice is running as a browser preview without the desktop shell.';

/** Waits until the message at `index` shows its full reply and the composer can send again. */
async function expectReplyComplete(page: Page, index: number, text: string): Promise<void> {
  await expect(page.locator('.message').nth(index).locator('.text')).toHaveText(text, {
    timeout: 10_000
  });
  await expect(page.locator('.message').nth(index).locator('.cursor')).toHaveCount(0);
  await expect(page.getByRole('combobox', { name: 'Provider' })).toBeEnabled();
}

async function addCredential(page: Page, label: string): Promise<void> {
  await page.getByRole('textbox', { name: 'Label' }).fill(label);
  await page.getByRole('textbox', { name: 'Credential key' }).fill('openai');
  await page.getByRole('button', { name: 'Add', exact: true }).click();
  await expect(page.locator('.credential-card').getByText(label, { exact: true })).toBeVisible();
}

async function addRemoteProvider(
  page: Page,
  label: string,
  endpoint: string,
  modelKey: string
): Promise<void> {
  await page.getByRole('textbox', { name: 'Provider name' }).fill(label);
  await page.getByRole('textbox', { name: 'Endpoint', exact: true }).fill(endpoint);
  await page.getByRole('textbox', { name: 'Model name', exact: true }).fill(modelKey);
  await page.getByRole('combobox', { name: 'Credential', exact: true }).selectOption({
    label: 'Remote key'
  });
  await page.getByRole('button', { name: 'Add provider' }).click();
  await expect(page.locator('.provider-card').getByText(label, { exact: true })).toBeVisible();
}

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
