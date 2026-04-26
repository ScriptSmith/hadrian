import { describe, it, expect } from "vitest";
import { SseParser } from "../sseParser";

describe("SseParser", () => {
  it("parses single-line data events with \\n terminator", () => {
    const parser = new SseParser();
    const events = [...parser.feed('data: {"hello": "world"}\n\n'), ...parser.flush()];
    expect(events).toEqual([
      { data: '{"hello": "world"}', event: "message", id: undefined, retry: undefined },
    ]);
  });

  it("handles \\r\\n line terminators", () => {
    const parser = new SseParser();
    const events = [...parser.feed("data: alpha\r\n\r\n"), ...parser.flush()];
    expect(events).toEqual([{ data: "alpha", event: "message", id: undefined, retry: undefined }]);
  });

  it("handles bare \\r line terminators", () => {
    const parser = new SseParser();
    const events = [...parser.feed("data: line\r\r"), ...parser.flush()];
    expect(events.map((e) => e.data)).toEqual(["line"]);
  });

  it("joins multi-line data fields with \\n", () => {
    const parser = new SseParser();
    const events = [...parser.feed("data: line1\ndata: line2\ndata: line3\n\n"), ...parser.flush()];
    expect(events[0].data).toBe("line1\nline2\nline3");
  });

  it("dispatches only on blank line", () => {
    const parser = new SseParser();
    // First chunk has no blank line — nothing should emit yet.
    const partial = [...parser.feed('data: {"a":1}\n')];
    expect(partial).toEqual([]);
    // Second chunk completes the event.
    const completed = [...parser.feed("data: more\n\n")];
    expect(completed.map((e) => e.data)).toEqual(['{"a":1}\nmore']);
  });

  it("handles chunked input with split mid-line", () => {
    const parser = new SseParser();
    const out = [...parser.feed('data: {"par'), ...parser.feed('tial": true}\n\n')];
    expect(out.map((e) => e.data)).toEqual(['{"partial": true}']);
  });

  it("ignores comment lines", () => {
    const parser = new SseParser();
    const events = [...parser.feed(": keep-alive\ndata: payload\n\n")];
    expect(events.map((e) => e.data)).toEqual(["payload"]);
  });

  it("captures event name and id", () => {
    const parser = new SseParser();
    const events = [...parser.feed("event: ping\nid: 42\ndata: hi\n\n")];
    expect(events).toEqual([{ data: "hi", event: "ping", id: "42", retry: undefined }]);
  });

  it("flush emits unterminated trailing event", () => {
    const parser = new SseParser();
    const buffered = [...parser.feed("data: trailing")];
    expect(buffered).toEqual([]);
    const flushed = [...parser.flush()];
    expect(flushed.map((e) => e.data)).toEqual(["trailing"]);
  });

  it("treats blank-only input as keep-alive (no events)", () => {
    const parser = new SseParser();
    const events = [...parser.feed("\n\n\n")];
    expect(events).toEqual([]);
  });
});
