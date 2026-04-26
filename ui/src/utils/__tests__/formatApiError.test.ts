import { describe, it, expect } from "vitest";
import { formatApiError } from "../formatApiError";

describe("formatApiError", () => {
  it("passes strings through", () => {
    expect(formatApiError("boom")).toBe("boom");
  });

  it("falls back to 'Unknown error' on null/undefined", () => {
    expect(formatApiError(null)).toBe("Unknown error");
    expect(formatApiError(undefined)).toBe("Unknown error");
  });

  it("uses Error.message", () => {
    expect(formatApiError(new Error("nope"))).toBe("nope");
  });

  it("prefers an API body shape on Error objects", () => {
    const err = Object.assign(new Error("HTTP 400"), { body: { message: "bad input" } });
    expect(formatApiError(err)).toBe("bad input");
  });

  it("walks { error: { message } } envelopes", () => {
    expect(formatApiError({ error: { message: "denied" } })).toBe("denied");
  });

  it("walks { error: 'string' } envelopes", () => {
    expect(formatApiError({ error: "denied" })).toBe("denied");
  });

  it("falls back to Unknown error rather than [object Object]", () => {
    expect(formatApiError({ random: 1 })).toBe("Unknown error");
  });
});
