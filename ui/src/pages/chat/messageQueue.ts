import type { ChatFile, QueuedMessage } from "@/components/chat-types";

export type SendFn = (content: string, files: ChatFile[]) => Promise<void>;

/**
 * Serializes chat sends so the user can keep typing (and hit "send") while a
 * response is still streaming.
 *
 * - An **idle** send goes out immediately — identical to sending without a
 *   queue at all.
 * - A send issued **while a turn is in flight** is appended to the queue and
 *   dispatched one at a time as each turn completes.
 *
 * The serialization signal is the `send` promise itself, which (see `useChat`)
 * resolves only when the whole turn is done — including multi-round tool
 * execution. `isStreaming` can briefly flip false *between* tool rounds, so it
 * is deliberately not used to drive the queue here.
 *
 * This is used as an app-wide singleton (see `chatMessageQueue`) rather than
 * component state. The first message of a conversation navigates `/chat` →
 * `/chat/:id`, which remounts `ChatPage`; component-scoped state would reset the
 * in-flight lock and let a queued message start a second turn *concurrently*
 * with the one still streaming (two side-by-side responses for one model). A
 * singleton keeps the lock alive across that remount.
 *
 * `send` is kept current via {@link setSend} (called from an effect in
 * `useMessageQueue`) so each dispatch uses the latest `sendMessage` closure,
 * picking up model/tool/config changes the user made while the queue was
 * draining.
 */
export class MessageQueue {
  private queue: QueuedMessage[] = [];
  private busy = false;
  private send: SendFn | null = null;
  private readonly listeners = new Set<(queue: QueuedMessage[]) => void>();
  private readonly busyListeners = new Set<(busy: boolean) => void>();

  /** Current number of queued (not-yet-dispatched) messages. */
  get size(): number {
    return this.queue.length;
  }

  /** True while a turn is being sent (and therefore further sends will queue). */
  get isBusy(): boolean {
    return this.busy;
  }

  // Flip the busy flag and notify subscribers. The UI uses this to keep the
  // Send/Queue label in step with the actual dispatch decision: `busy` stays
  // true across a whole turn (including between tool rounds, where `isStreaming`
  // briefly flips false), so a click always matches the rendered label.
  private setBusy(value: boolean) {
    if (this.busy === value) return;
    this.busy = value;
    for (const listener of this.busyListeners) listener(value);
  }

  /** Point the queue at the current send function. Call on every render. */
  setSend(send: SendFn) {
    this.send = send;
  }

  /** Subscribe to queue changes. The listener is invoked immediately with the
   *  current queue, and on every subsequent change. Returns an unsubscribe. */
  subscribe(listener: (queue: QueuedMessage[]) => void): () => void {
    this.listeners.add(listener);
    listener([...this.queue]);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Subscribe to busy-state changes. The listener is invoked immediately with
   *  the current value, and on every subsequent change. Returns an unsubscribe. */
  subscribeBusy(listener: (busy: boolean) => void): () => void {
    this.busyListeners.add(listener);
    listener(this.busy);
    return () => {
      this.busyListeners.delete(listener);
    };
  }

  private emit() {
    const snapshot = [...this.queue];
    for (const listener of this.listeners) listener(snapshot);
  }

  /** Send now if idle, otherwise queue for after the in-flight turn completes. */
  sendOrQueue(content: string, files: ChatFile[]) {
    if (this.busy) {
      this.queue = [...this.queue, { id: crypto.randomUUID(), content, files }];
      this.emit();
      return;
    }
    void this.pump(content, files);
  }

  /** Remove a queued message before it is dispatched. */
  remove(id: string) {
    const next = this.queue.filter((m) => m.id !== id);
    if (next.length === this.queue.length) return;
    this.queue = next;
    this.emit();
  }

  /** Drop all queued messages (e.g. when switching conversations). Does not
   *  affect a turn already in flight. */
  clear() {
    if (this.queue.length === 0) return;
    this.queue = [];
    this.emit();
  }

  // Dispatch `first`, then keep draining the queue until it is empty. `busy`
  // stays true across the whole drain so no other send can start concurrently
  // (which would clobber the in-flight stream).
  private async pump(first: string, firstFiles: ChatFile[]) {
    this.setBusy(true);
    try {
      await this.runSafe(first, firstFiles);
      while (this.queue.length > 0) {
        const [next, ...rest] = this.queue;
        this.queue = rest;
        this.emit();
        await this.runSafe(next.content, next.files);
      }
    } finally {
      this.setBusy(false);
    }
  }

  // A single send that never rejects — a failed turn must not strand the rest
  // of the queue (or leave `busy` stuck true).
  private async runSafe(content: string, files: ChatFile[]) {
    const send = this.send;
    if (!send) return;
    try {
      await send(content, files);
    } catch (err) {
      console.error("Chat message failed to send:", err);
    }
  }
}

/**
 * App-wide chat send queue. A singleton (not component state) so the in-flight
 * lock survives the `ChatPage` remount that happens when the first message of a
 * conversation navigates `/chat` → `/chat/:id`.
 */
export const chatMessageQueue = new MessageQueue();
