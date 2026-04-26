/**
 * Minimal SSE parser following the WHATWG EventSource spec, used by the
 * streaming chat client.
 *
 * The previous parser called `buffer.split("\n")` and treated every
 * `data: ...` line as a complete event. That breaks on:
 *   - servers that emit `\r\n` (or `\r`) line terminators,
 *   - events that span multiple `data:` lines (the spec says concatenate
 *     them with `\n`),
 *   - producers that rely on the spec's "events end on a blank line"
 *     semantics (we'd emit half-events early).
 *
 * Usage:
 *   const parser = new SseParser();
 *   for (const chunk of stream) {
 *     for (const ev of parser.feed(chunk)) {
 *       handle(ev);
 *     }
 *   }
 *   for (const ev of parser.flush()) handle(ev); // flush trailing event
 */

export interface SseEvent {
  /** Concatenated `data:` fields, joined with `\n`. Empty string if none. */
  data: string;
  /** `event:` field, or `"message"` if absent (per spec). */
  event: string;
  /** `id:` field, if present. */
  id?: string;
  /** `retry:` reconnect time in ms, if present. */
  retry?: number;
}

export class SseParser {
  private buffer = "";
  private dataLines: string[] = [];
  private eventName = "";
  private lastEventId: string | undefined;
  private retry: number | undefined;

  /**
   * Append `chunk` to the buffer and yield any complete events that
   * become available. Trailing partial lines are kept buffered until the
   * next call.
   */
  *feed(chunk: string): Generator<SseEvent> {
    this.buffer += chunk;
    // Spec: events are separated by `\r\n`, `\r`, or `\n`. Use a regex
    // that matches any of them.
    let newlineIdx: number;
    while ((newlineIdx = this.buffer.search(/\r\n|\r|\n/)) !== -1) {
      const line = this.buffer.slice(0, newlineIdx);
      const sepLen =
        this.buffer.charAt(newlineIdx) === "\r" && this.buffer.charAt(newlineIdx + 1) === "\n"
          ? 2
          : 1;
      this.buffer = this.buffer.slice(newlineIdx + sepLen);

      if (line === "") {
        // Blank line: dispatch the accumulated event, if any.
        const ev = this.dispatch();
        if (ev) yield ev;
        continue;
      }

      this.processField(line);
    }
  }

  /**
   * Emit any pending event that hasn't been terminated by a blank line.
   * Use at end-of-stream so a producer that closes without a trailing
   * blank line still surfaces its last event.
   */
  *flush(): Generator<SseEvent> {
    if (this.buffer.length > 0) {
      // Treat the trailing partial line as a final field.
      this.processField(this.buffer);
      this.buffer = "";
    }
    const ev = this.dispatch();
    if (ev) yield ev;
  }

  private processField(line: string) {
    // Comment lines start with ":" per spec — ignore.
    if (line.startsWith(":")) return;

    const colon = line.indexOf(":");
    let field: string;
    let value: string;
    if (colon === -1) {
      field = line;
      value = "";
    } else {
      field = line.slice(0, colon);
      value = line.slice(colon + 1);
      // Per spec: a single leading space in the value is removed.
      if (value.startsWith(" ")) value = value.slice(1);
    }

    switch (field) {
      case "data":
        this.dataLines.push(value);
        break;
      case "event":
        this.eventName = value;
        break;
      case "id":
        // Per spec: ignore IDs containing NUL.
        if (!value.includes("\0")) this.lastEventId = value;
        break;
      case "retry": {
        const n = Number(value);
        if (Number.isFinite(n) && n >= 0) this.retry = n;
        break;
      }
      // Unknown fields are silently ignored.
    }
  }

  private dispatch(): SseEvent | null {
    if (this.dataLines.length === 0 && this.eventName === "") {
      // Nothing buffered — happens for keep-alive blank lines.
      this.resetEventState();
      return null;
    }
    const ev: SseEvent = {
      data: this.dataLines.join("\n"),
      event: this.eventName || "message",
      id: this.lastEventId,
      retry: this.retry,
    };
    this.resetEventState();
    return ev;
  }

  private resetEventState() {
    this.dataLines = [];
    this.eventName = "";
    // Per spec, `id` and `retry` persist across events; only data/event reset.
  }
}
