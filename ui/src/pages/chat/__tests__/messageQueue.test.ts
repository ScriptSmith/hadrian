import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

import type { QueuedMessage } from "@/components/chat-types";
import { MessageQueue, type SendFn } from "../messageQueue";

/** A send whose promise we resolve manually, so we can interleave events. */
function deferred() {
  let resolve!: () => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("MessageQueue", () => {
  let lastQueue: QueuedMessage[] = [];
  const onChange = (q: QueuedMessage[]) => {
    lastQueue = q;
  };

  /** Build a queue wired to a send function and the shared `onChange` spy. */
  function makeQueue(send: SendFn) {
    const q = new MessageQueue();
    q.subscribe(onChange);
    q.setSend(send);
    return q;
  }

  beforeEach(() => {
    lastQueue = [];
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("sends immediately when idle and shows no queue", () => {
    const send = vi.fn().mockResolvedValue(undefined);
    const q = makeQueue(send);

    q.sendOrQueue("hello", []);

    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith("hello", []);
    // Idle send is never added to the visible queue.
    expect(lastQueue).toEqual([]);
  });

  it("queues a second send while the first is in flight and dispatches it only after the first resolves", async () => {
    const order: string[] = [];
    const first = deferred();
    const second = deferred();
    const send = vi
      .fn()
      .mockImplementationOnce(async (content: string) => {
        order.push(`start:${content}`);
        await first.promise;
        order.push(`end:${content}`);
      })
      .mockImplementationOnce(async (content: string) => {
        order.push(`start:${content}`);
        await second.promise;
        order.push(`end:${content}`);
      });

    const q = makeQueue(send);

    q.sendOrQueue("one", []); // idle -> sends immediately
    q.sendOrQueue("two", []); // busy -> queued

    // Second send must NOT have started yet — the first turn is still streaming.
    expect(send).toHaveBeenCalledTimes(1);
    expect(q.size).toBe(1);
    expect(lastQueue.map((m) => m.content)).toEqual(["two"]);

    // Finish the first turn; the queued message is dispatched next.
    first.resolve();
    await flush();

    expect(send).toHaveBeenCalledTimes(2);
    expect(q.size).toBe(0);
    expect(lastQueue).toEqual([]);

    second.resolve();
    await flush();

    // Strictly serialized: never overlapping.
    expect(order).toEqual(["start:one", "end:one", "start:two", "end:two"]);
  });

  it("preserves order across several queued messages", async () => {
    const gates = [deferred(), deferred(), deferred()];
    let i = 0;
    const started: string[] = [];
    const send = vi.fn().mockImplementation(async (content: string) => {
      started.push(content);
      await gates[i++].promise;
    });

    const q = makeQueue(send);

    q.sendOrQueue("a", []);
    q.sendOrQueue("b", []);
    q.sendOrQueue("c", []);

    expect(started).toEqual(["a"]);

    gates[0].resolve();
    await flush();
    expect(started).toEqual(["a", "b"]);

    gates[1].resolve();
    await flush();
    expect(started).toEqual(["a", "b", "c"]);

    gates[2].resolve();
    await flush();
    expect(q.size).toBe(0);
  });

  it("does not strand the queue when a send rejects", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const first = deferred();
    const send = vi
      .fn()
      .mockImplementationOnce(async () => {
        await first.promise;
        throw new Error("boom");
      })
      .mockResolvedValue(undefined);

    const q = makeQueue(send);

    q.sendOrQueue("one", []);
    q.sendOrQueue("two", []);
    expect(q.size).toBe(1);

    first.resolve(); // gate opens, then the first turn throws "boom"
    await flush();

    // Despite the first turn failing, the queued message is still dispatched
    // and the queue is no longer busy.
    expect(send).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenLastCalledWith("two", []);
    expect(q.size).toBe(0);
    expect(q.isBusy).toBe(false);
    errorSpy.mockRestore();
  });

  it("removes a queued message before it is dispatched", async () => {
    const first = deferred();
    const send = vi
      .fn()
      .mockImplementationOnce(async () => {
        await first.promise;
      })
      .mockResolvedValue(undefined);

    const q = makeQueue(send);

    q.sendOrQueue("one", []);
    q.sendOrQueue("two", []);
    const queuedId = lastQueue[0].id;
    expect(q.size).toBe(1);

    q.remove(queuedId);
    expect(q.size).toBe(0);
    expect(lastQueue).toEqual([]);

    first.resolve();
    await flush();

    // The removed message is never sent (only the idle "one").
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("clear() drops queued messages without affecting the in-flight turn", async () => {
    const first = deferred();
    const send = vi
      .fn()
      .mockImplementationOnce(async () => {
        await first.promise;
      })
      .mockResolvedValue(undefined);

    const q = makeQueue(send);

    q.sendOrQueue("one", []);
    q.sendOrQueue("two", []);
    q.sendOrQueue("three", []);
    expect(q.size).toBe(2);

    q.clear();
    expect(q.size).toBe(0);
    expect(lastQueue).toEqual([]);

    first.resolve();
    await flush();

    // Only the in-flight "one" ran; the cleared messages did not.
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("models the streaming commit order without clobbering an in-flight turn", async () => {
    // Simulate the real send: append user msg synchronously, start "streaming",
    // and append the assistant msg when the turn resolves.
    const transcript: string[] = [];
    let streaming = false;
    const gate1 = deferred();
    const gate2 = deferred();
    const gates = [gate1, gate2];
    let turn = 0;

    const send = vi.fn().mockImplementation(async (content: string) => {
      // No two turns may overlap (would reset the streaming store).
      expect(streaming).toBe(false);
      transcript.push(`user:${content}`);
      streaming = true;
      const g = gates[turn++];
      await g.promise;
      transcript.push(`assistant:${content}`);
      streaming = false;
    });

    const q = makeQueue(send);

    q.sendOrQueue("q1", []);
    // User queues q2 while q1 is still streaming.
    q.sendOrQueue("q2", []);
    expect(transcript).toEqual(["user:q1"]);

    gate1.resolve();
    await flush();
    gate2.resolve();
    await flush();

    expect(transcript).toEqual(["user:q1", "assistant:q1", "user:q2", "assistant:q2"]);
  });

  it("uses the latest send function for each dispatch", async () => {
    const first = deferred();
    const sendA = vi.fn().mockImplementation(async () => {
      await first.promise;
    });
    const sendB = vi.fn().mockResolvedValue(undefined);

    const q = makeQueue(sendA);

    q.sendOrQueue("one", []);
    q.sendOrQueue("two", []);

    // Config changes while the queue drains -> point at the new send function.
    q.setSend(sendB);

    first.resolve();
    await flush();

    expect(sendA).toHaveBeenCalledTimes(1);
    expect(sendB).toHaveBeenCalledTimes(1);
    expect(sendB).toHaveBeenCalledWith("two", []);
  });
});
