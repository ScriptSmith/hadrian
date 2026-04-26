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
    // Spec: events are separated by `\r\n`, `\r`, or `\n`. We scan for any
    // of those, but if the buffer ends on a lone `\r` we leave it in place
    // until the next chunk arrives — otherwise a chunk boundary that splits
    // a `\r\n` would be misread as `\r` followed by an empty `\n`-terminated
    // line, which would emit a spurious blank-line dispatch on the next
    // feed and prematurely complete an in-flight event.
    while (true) {
      const sepStart = this.buffer.search(/\r\n|\r|\n/);
      if (sepStart === -1) break;
      let sepLen: number;
      if (this.buffer.charAt(sepStart) === "\r") {
        if (sepStart === this.buffer.length - 1) {
          // Lone trailing `\r` — could still pair with a `\n` in the next
          // chunk. Defer until we see what follows.
          break;
        }
        sepLen = this.buffer.charAt(sepStart + 1) === "\n" ? 2 : 1;
      } else {
        sepLen = 1;
      }
      const line = this.buffer.slice(0, sepStart);
      this.buffer = this.buffer.slice(sepStart + sepLen);

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
