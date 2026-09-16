import type { ChatMessage } from '@lattice/types';

import {
  beginOrContinueWebConversation,
  checkpointWebAssistantMessage,
  deleteWebConversation,
  finalizeWebAssistantMessage,
  getWebConversation,
  listWebConversations,
  resetWebConversationsForTest,
  startWebAssistantMessage
} from './conversations-fallback';

function userMessage(text: string): ChatMessage {
  return { role: 'user', text };
}

describe('conversations browser fallback', () => {
  beforeEach(() => {
    resetWebConversationsForTest();
  });

  it('creates a conversation on first message and derives its title', () => {
    const id = beginOrContinueWebConversation(null, [userMessage('Hello there')]);

    const detail = getWebConversation({ conversationId: id });
    expect(detail.conversation.title).toBe('Hello there');
    expect(detail.messages).toEqual([
      expect.objectContaining({
        role: 'user',
        text: 'Hello there',
        status: 'complete',
        sequence: 0
      })
    ]);
  });

  it('appends only new trailing messages when continuing a conversation', () => {
    const id = beginOrContinueWebConversation(null, [userMessage('first')]);
    beginOrContinueWebConversation(id, [
      userMessage('first'),
      { role: 'assistant', text: 'reply' },
      userMessage('second')
    ]);

    const detail = getWebConversation({ conversationId: id });
    expect(detail.messages).toHaveLength(3);
    expect(detail.messages[2]).toEqual(expect.objectContaining({ text: 'second', sequence: 2 }));
  });

  it('refuses to continue an unknown conversation', () => {
    expect(() => beginOrContinueWebConversation('missing', [userMessage('hi')])).toThrow(
      expect.objectContaining({ code: 'conversation.not_found' })
    );
  });

  it('refuses stale, shorter local history as a conflict', () => {
    const id = beginOrContinueWebConversation(null, [
      userMessage('first'),
      { role: 'assistant', text: 'reply' }
    ]);

    expect(() => beginOrContinueWebConversation(id, [userMessage('only one')])).toThrow(
      expect.objectContaining({ code: 'conversation.conflict' })
    );
  });

  it('checkpoints then finalizes a streaming assistant message', () => {
    const id = beginOrContinueWebConversation(null, [userMessage('hi')]);
    const messageId = startWebAssistantMessage(
      id,
      'local-openai-compatible',
      'qwen/qwen2.5-0.5b-instruct'
    );

    expect(getWebConversation({ conversationId: id }).messages[1]).toEqual(
      expect.objectContaining({ status: 'streaming', text: '' })
    );

    checkpointWebAssistantMessage(messageId, 'Hello');
    finalizeWebAssistantMessage(messageId, 'Hello, world!', 'complete');

    const finished = getWebConversation({ conversationId: id }).messages[1];
    expect(finished).toEqual(
      expect.objectContaining({
        status: 'complete',
        text: 'Hello, world!',
        modelKey: 'qwen/qwen2.5-0.5b-instruct'
      })
    );
  });

  it('lists conversations most recently updated first and paginates', () => {
    const ids = ['a', 'b', 'c'].map((label) =>
      beginOrContinueWebConversation(null, [userMessage(label)])
    );

    const firstPage = listWebConversations({ limit: 2 });
    expect(firstPage.conversations.map((conversation) => conversation.id)).toEqual([
      ids[2],
      ids[1]
    ]);
    expect(firstPage.nextBefore).toBeDefined();

    const secondPage = listWebConversations({
      limit: 2,
      ...(firstPage.nextBefore ? { before: firstPage.nextBefore } : {})
    });
    expect(secondPage.conversations.map((conversation) => conversation.id)).toEqual([ids[0]]);
    expect(secondPage.nextBefore).toBeUndefined();
  });

  it('deletes a conversation without affecting others', () => {
    const keep = beginOrContinueWebConversation(null, [userMessage('keep')]);
    const removeId = beginOrContinueWebConversation(null, [userMessage('remove')]);

    deleteWebConversation({ conversationId: removeId });

    expect(() => getWebConversation({ conversationId: removeId })).toThrow(
      expect.objectContaining({ code: 'conversation.not_found' })
    );
    expect(getWebConversation({ conversationId: keep }).messages).toHaveLength(1);
  });

  it('refuses to delete an unknown conversation', () => {
    expect(() => deleteWebConversation({ conversationId: 'missing' })).toThrow(
      expect.objectContaining({ code: 'conversation.not_found' })
    );
  });
});
