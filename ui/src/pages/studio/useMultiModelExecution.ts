import { useState, useCallback, useRef } from "react";
import type { ModelInstance } from "@/components/chat-types";

export interface ExecutionResult<T> {
  data: T;
  costMicrocents?: number;
}

export interface InstanceResult<T> {
  instanceId: string;
  modelId: string;
  label?: string;
  status: "loading" | "complete" | "error";
  data?: T;
  error?: string;
  durationMs?: number;
  costMicrocents?: number;
}

/** Extract cost from an API response's X-Cost-Microcents header */
export function extractCostFromResponse(response?: Response): number | undefined {
  const raw = response?.headers?.get("X-Cost-Microcents");
  if (!raw) return undefined;
  const parsed = parseInt(raw, 10);
  return Number.isFinite(parsed) ? parsed : undefined;
}

export function useMultiModelExecution<T>() {
  const [results, setResults] = useState<Map<string, InstanceResult<T>>>(new Map());
  const [isExecuting, setIsExecuting] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  const execute = useCallback(
    async (
      instances: ModelInstance[],
      callFn: (instance: ModelInstance, signal: AbortSignal) => Promise<ExecutionResult<T>>
    ): Promise<InstanceResult<T>[]> => {
      // Abort any in-flight execution
      abortRef.current?.abort();
      const controller = new AbortController();
      abortRef.current = controller;

      setIsExecuting(true);

      // Initialize all results as loading
      const initial = new Map<string, InstanceResult<T>>();
      for (const inst of instances) {
        initial.set(inst.id, {
          instanceId: inst.id,
          modelId: inst.modelId,
          label: inst.label,
          status: "loading",
        });
      }
      setResults(new Map(initial));

      const settled = await Promise.allSettled(
        instances.map(async (inst) => {
          const start = performance.now();
          try {
            const { data, costMicrocents } = await callFn(inst, controller.signal);
            const durationMs = Math.round(performance.now() - start);
            const result: InstanceResult<T> = {
              instanceId: inst.id,
              modelId: inst.modelId,
              label: inst.label,
              status: "complete",
              data,
              durationMs,
              costMicrocents,
            };
            if (!controller.signal.aborted) {
              setResults((prev) => {
                const next = new Map(prev);
                next.set(inst.id, result);
                return next;
              });
            }
            return result;
          } catch (err) {
            const durationMs = Math.round(performance.now() - start);
            const result: InstanceResult<T> = {
              instanceId: inst.id,
              modelId: inst.modelId,
              label: inst.label,
              status: "error",
              error: err instanceof Error ? err.message : "Unknown error",
              durationMs,
            };
            if (!controller.signal.aborted) {
              setResults((prev) => {
                const next = new Map(prev);
                next.set(inst.id, result);
                return next;
              });
            }
            return result;
          }
        })
      );

      if (!controller.signal.aborted) {
        setIsExecuting(false);
      }

      return settled.map((s) => (s.status === "fulfilled" ? s.value : s.reason));
    },
    []
  );

  const clearResults = useCallback(() => {
    abortRef.current?.abort();
    setResults(new Map());
    setIsExecuting(false);
  }, []);

  return { isExecuting, results, execute, clearResults };
}
