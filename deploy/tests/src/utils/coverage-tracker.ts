/**
 * API Coverage Tracker
 *
 * Tracks which API endpoints and parameters are exercised by the test suite,
 * compared against the OpenAPI spec.
 *
 * Note: Vitest runs tests in worker threads, separate from the reporter in the
 * main thread. To share coverage data reliably across parallel workers, each
 * worker writes to its own temp file (keyed by process ID). The reporter
 * aggregates all worker files at the end.
 */
import type { OpenAPIV3_1 } from "openapi-types";
import * as fs from "node:fs/promises";
import * as fsSync from "node:fs";
import * as path from "node:path";

// Coverage temp files use a per-worker pattern to avoid write conflicts
// Format: .coverage-data-{pid}.jsonl
const COVERAGE_TEMP_DIR = `${process.cwd()}/coverage`;
const COVERAGE_TEMP_PREFIX = ".coverage-data-";
const COVERAGE_TEMP_SUFFIX = ".jsonl";

/** Ensure the coverage temp directory exists (synchronous, idempotent). */
function ensureCoverageTempDir(): void {
  if (!fsSync.existsSync(COVERAGE_TEMP_DIR)) {
    fsSync.mkdirSync(COVERAGE_TEMP_DIR, { recursive: true });
  }
}

/**
 * Get the temp file path for the current worker.
 * Uses process.pid for uniqueness across worker threads.
 */
function getWorkerTempFile(): string {
  return path.resolve(
    COVERAGE_TEMP_DIR,
    `${COVERAGE_TEMP_PREFIX}${process.pid}${COVERAGE_TEMP_SUFFIX}`,
  );
}

/**
 * Get glob pattern for all coverage temp files.
 */
function getCoverageTempPattern(): string {
  return `${COVERAGE_TEMP_PREFIX}*${COVERAGE_TEMP_SUFFIX}`;
}

/** Serializable API call record for file storage */
interface ApiCallRecord {
  method: string;
  pathPattern: string;
  pathParams?: string[];
  queryParams?: string[];
  bodyParams?: string[];
  statusCode?: number;
}

/** Coverage data for a single endpoint (aggregated) */
interface EndpointCoverage {
  method: string;
  pathPattern: string;
  pathParams: Set<string>;
  queryParams: Set<string>;
  bodyParams: Set<string>;
  statusCodes: Set<number>;
  callCount: number;
}

/** Summary metrics for the coverage report */
interface CoverageSummary {
  covered: number;
  total: number;
  percentage: number;
}

/** Detailed endpoint info for the report */
interface EndpointDetail {
  method: string;
  path: string;
  operationId?: string;
  pathParamsCovered?: string[];
  pathParamsTotal?: string[];
  queryParamsCovered?: string[];
  queryParamsTotal?: string[];
  bodyParamsCovered?: string[];
  bodyParamsTotal?: string[];
  statusCodesCovered?: number[];
  statusCodesTotal?: string[];
  callCount?: number;
}

/** Metadata about the coverage data loading process */
interface LoadingMetadata {
  workerFilesLoaded: number;
  recordsLoaded: number;
  loadErrors: string[];
}

/** Full coverage report structure */
export interface CoverageReport {
  summary: {
    endpoints: CoverageSummary;
    parameters: CoverageSummary;
    statusCodes: CoverageSummary;
  };
  loading: LoadingMetadata;
  endpoints: {
    covered: EndpointDetail[];
    uncovered: EndpointDetail[];
  };
  generatedAt: string;
}

/** Path pattern matcher for URL normalization */
interface PathMatcher {
  pattern: string;
  regex: RegExp;
  paramNames: string[];
}

/**
 * Singleton class to track API calls and generate coverage reports.
 */
class ApiCoverageTracker {
  private static instance: ApiCoverageTracker;
  private calls = new Map<string, EndpointCoverage>();
  private openApiSpec: OpenAPIV3_1.Document | null = null;
  private pathMatchers: PathMatcher[] = [];
  private specLoaded = false;

  // Write buffering to reduce filesystem calls
  private writeBuffer: ApiCallRecord[] = [];
  private flushTimeout: ReturnType<typeof setTimeout> | null = null;
  private readonly BUFFER_SIZE = 50;
  private readonly FLUSH_INTERVAL_MS = 1000;
  private exitHandlerRegistered = false;

  private constructor() {}

  static getInstance(): ApiCoverageTracker {
    if (!ApiCoverageTracker.instance) {
      ApiCoverageTracker.instance = new ApiCoverageTracker();
    }
    return ApiCoverageTracker.instance;
  }

  /**
   * Load the OpenAPI spec and build path matchers.
   */
  async loadSpec(specPath: string): Promise<void> {
    if (this.specLoaded) return;

    const content = await fs.readFile(specPath, "utf-8");
    this.openApiSpec = JSON.parse(content) as OpenAPIV3_1.Document;
    this.buildPathMatchers();
    this.specLoaded = true;
  }

  /**
   * Build regex matchers for all paths in the OpenAPI spec.
   * Converts path patterns like /admin/v1/users/{user_id} to regexes.
   */
  private buildPathMatchers(): void {
    if (!this.openApiSpec?.paths) return;

    for (const pathPattern of Object.keys(this.openApiSpec.paths)) {
      const paramNames: string[] = [];

      // Extract parameter names and build regex
      // e.g., /admin/v1/users/{user_id} -> /^\/admin\/v1\/users\/([^\/]+)$/
      const regexPattern = pathPattern.replace(
        /\{([^}]+)\}/g,
        (_match, paramName) => {
          paramNames.push(paramName);
          return "([^/]+)";
        },
      );

      this.pathMatchers.push({
        pattern: pathPattern,
        regex: new RegExp(`^${regexPattern}$`),
        paramNames,
      });
    }

    // Sort by specificity (more segments = more specific, match first)
    this.pathMatchers.sort((a, b) => {
      const aSegments = a.pattern.split("/").length;
      const bSegments = b.pattern.split("/").length;
      return bSegments - aSegments;
    });
  }

  /**
   * Find the matching path pattern for a concrete URL.
   */
  private matchPath(pathname: string): PathMatcher | null {
    for (const matcher of this.pathMatchers) {
      if (matcher.regex.test(pathname)) {
        return matcher;
      }
    }
    return null;
  }

  /** Track write errors for reporting */
  private writeErrors: Array<{ file: string; error: string }> = [];

  /**
   * Register a process exit handler to flush any remaining buffered data.
   * Uses 'exit' event which allows synchronous operations (appendFileSync).
   */
  private registerExitHandler(): void {
    if (this.exitHandlerRegistered) return;
    this.exitHandlerRegistered = true;
    process.on("exit", () => this.flushWriteBuffer());
  }

  /**
   * Add an API call record to the write buffer.
   * Buffer is flushed when it reaches BUFFER_SIZE or after FLUSH_INTERVAL_MS.
   */
  private persistCall(record: ApiCallRecord): void {
    this.registerExitHandler();
    this.writeBuffer.push(record);

    // Flush immediately if buffer is full
    if (this.writeBuffer.length >= this.BUFFER_SIZE) {
      this.flushWriteBuffer();
    } else if (!this.flushTimeout) {
      // Schedule a flush if not already scheduled
      this.flushTimeout = setTimeout(
        () => this.flushWriteBuffer(),
        this.FLUSH_INTERVAL_MS
      );
    }
  }

  /**
   * Flush the write buffer to disk.
   * Writes all buffered records in a single filesystem call for efficiency.
   */
  private flushWriteBuffer(): void {
    // Clear the timeout if it's pending
    if (this.flushTimeout) {
      clearTimeout(this.flushTimeout);
      this.flushTimeout = null;
    }

    if (this.writeBuffer.length === 0) {
      return;
    }

    const tempFile = getWorkerTempFile();
    const records = this.writeBuffer;
    this.writeBuffer = [];

    try {
      ensureCoverageTempDir();
      const lines = records.map((r) => JSON.stringify(r)).join("\n") + "\n";
      fsSync.appendFileSync(tempFile, lines, "utf-8");
    } catch (err) {
      // Track write errors for reporting, but don't break tests
      const errorMsg = err instanceof Error ? err.message : String(err);
      this.writeErrors.push({ file: tempFile, error: errorMsg });
      // Log immediately so issues are visible during test runs
      console.error(`[API Coverage] Write error to ${tempFile}: ${errorMsg}`);
    }
  }

  /**
   * Flush any pending writes. Call during test teardown to ensure all data is written.
   */
  flush(): void {
    this.flushWriteBuffer();
  }

  /**
   * Get any write errors that occurred during this session.
   */
  getWriteErrors(): Array<{ file: string; error: string }> {
    return [...this.writeErrors];
  }

  /**
   * Record an API call from the SDK client interceptor.
   * The interceptor provides the URL pattern directly from the SDK options.
   */
  recordFromInterceptor(
    method: string,
    pathPattern: string,
    pathParams?: Record<string, unknown>,
    queryParams?: Record<string, unknown>,
    body?: unknown,
    statusCode?: number,
  ): void {
    // Persist to file for cross-worker sharing
    const record: ApiCallRecord = {
      method: method.toUpperCase(),
      pathPattern,
      pathParams: pathParams ? Object.keys(pathParams) : undefined,
      queryParams: queryParams ? Object.keys(queryParams) : undefined,
      bodyParams:
        body && typeof body === "object" && !Array.isArray(body)
          ? Object.keys(body)
          : undefined,
      statusCode,
    };
    this.persistCall(record);

    // Also update in-memory state (for same-process access)
    this.mergeCall(record);
  }

  /**
   * Merge an API call record into the in-memory state.
   */
  private mergeCall(record: ApiCallRecord): void {
    const key = `${record.method} ${record.pathPattern}`;
    let entry = this.calls.get(key);

    if (!entry) {
      entry = {
        method: record.method,
        pathPattern: record.pathPattern,
        pathParams: new Set(),
        queryParams: new Set(),
        bodyParams: new Set(),
        statusCodes: new Set(),
        callCount: 0,
      };
      this.calls.set(key, entry);
    }

    entry.callCount++;

    if (record.pathParams) {
      for (const param of record.pathParams) {
        entry.pathParams.add(param);
      }
    }

    if (record.queryParams) {
      for (const param of record.queryParams) {
        entry.queryParams.add(param);
      }
    }

    if (record.bodyParams) {
      for (const param of record.bodyParams) {
        entry.bodyParams.add(param);
      }
    }

    if (record.statusCode !== undefined) {
      entry.statusCodes.add(record.statusCode);
    }
  }

  /**
   * Record an API call from direct fetch.
   * We need to match the URL against OpenAPI patterns.
   */
  recordFromFetch(
    method: string,
    url: string | URL,
    body?: unknown,
    statusCode?: number,
  ): void {
    const urlObj = typeof url === "string" ? new URL(url) : url;
    const pathname = urlObj.pathname;

    // Find matching path pattern
    const matcher = this.matchPath(pathname);
    if (!matcher) {
      // Unknown endpoint - record with concrete path for debugging
      this.recordFromInterceptor(
        method,
        pathname,
        undefined,
        undefined,
        body,
        statusCode,
      );
      return;
    }

    // Extract path parameter values from the URL
    const pathParams: Record<string, string> = {};
    const match = pathname.match(matcher.regex);
    if (match) {
      matcher.paramNames.forEach((name, index) => {
        pathParams[name] = match[index + 1];
      });
    }

    // Extract query parameters
    const queryParams: Record<string, string> = {};
    urlObj.searchParams.forEach((value, key) => {
      queryParams[key] = value;
    });

    this.recordFromInterceptor(
      method,
      matcher.pattern,
      pathParams,
      queryParams,
      body,
      statusCode,
    );
  }

  /**
   * Load all API call records from all worker temp files.
   * Called by the reporter before generating the report.
   */
  async loadFromFile(): Promise<{
    filesLoaded: number;
    recordsLoaded: number;
    errors: string[];
  }> {
    const result = { filesLoaded: 0, recordsLoaded: 0, errors: [] as string[] };

    // Find all coverage temp files
    const tempFiles = await ApiCoverageTracker.findAllTempFiles();

    for (const tempFile of tempFiles) {
      try {
        const content = await fs.readFile(tempFile, "utf-8");
        const lines = content.trim().split("\n").filter(Boolean);
        result.filesLoaded++;

        for (const line of lines) {
          try {
            const record = JSON.parse(line) as ApiCallRecord;
            this.mergeCall(record);
            result.recordsLoaded++;
          } catch (parseErr) {
            // Track parse errors for debugging
            const errorMsg =
              parseErr instanceof Error ? parseErr.message : String(parseErr);
            result.errors.push(`Parse error in ${tempFile}: ${errorMsg}`);
          }
        }
      } catch (readErr) {
        // Track read errors
        const errorMsg =
          readErr instanceof Error ? readErr.message : String(readErr);
        result.errors.push(`Read error for ${tempFile}: ${errorMsg}`);
      }
    }

    return result;
  }

  /**
   * Find all coverage temp files in the working directory.
   */
  static async findAllTempFiles(): Promise<string[]> {
    try {
      const files = await fs.readdir(COVERAGE_TEMP_DIR);
      return files
        .filter(
          (f) =>
            f.startsWith(COVERAGE_TEMP_PREFIX) &&
            f.endsWith(COVERAGE_TEMP_SUFFIX),
        )
        .map((f) => path.join(COVERAGE_TEMP_DIR, f));
    } catch {
      return [];
    }
  }

  /**
   * Clear all worker temp files. Should be called at the start of a test run.
   */
  static async clearTempFile(): Promise<{ filesCleared: number }> {
    const tempFiles = await ApiCoverageTracker.findAllTempFiles();
    let filesCleared = 0;

    for (const tempFile of tempFiles) {
      try {
        await fs.unlink(tempFile);
        filesCleared++;
      } catch {
        // File doesn't exist or can't be deleted - continue with others
      }
    }

    return { filesCleared };
  }

  /**
   * Get the temp file path for the current worker (for debugging).
   */
  static getTempFilePath(): string {
    return getWorkerTempFile();
  }

  /**
   * Get the pattern used to identify temp files (for debugging).
   */
  static getTempFilePattern(): string {
    return getCoverageTempPattern();
  }

  /**
   * Generate the coverage report comparing recorded calls against the OpenAPI spec.
   */
  async generateReport(specPath?: string): Promise<CoverageReport> {
    // Load any data from all worker temp files
    const loadResult = await this.loadFromFile();

    if (specPath) {
      await this.loadSpec(specPath);
    }

    if (!this.openApiSpec) {
      throw new Error("OpenAPI spec not loaded. Call loadSpec() first.");
    }

    const report: CoverageReport = {
      summary: {
        endpoints: { covered: 0, total: 0, percentage: 0 },
        parameters: { covered: 0, total: 0, percentage: 0 },
        statusCodes: { covered: 0, total: 0, percentage: 0 },
      },
      loading: {
        workerFilesLoaded: loadResult.filesLoaded,
        recordsLoaded: loadResult.recordsLoaded,
        loadErrors: loadResult.errors,
      },
      endpoints: {
        covered: [],
        uncovered: [],
      },
      generatedAt: new Date().toISOString(),
    };

    const methods = ["get", "post", "put", "patch", "delete"] as const;

    // Iterate through all endpoints in the spec
    for (const [pathPattern, pathItem] of Object.entries(
      this.openApiSpec.paths || {},
    )) {
      if (!pathItem) continue;

      for (const method of methods) {
        const operation = pathItem[method] as
          | OpenAPIV3_1.OperationObject
          | undefined;
        if (!operation) continue;

        report.summary.endpoints.total++;
        const key = `${method.toUpperCase()} ${pathPattern}`;
        const entry = this.calls.get(key);

        // Get all parameters from the operation
        const allParams = [
          ...((pathItem.parameters || []) as OpenAPIV3_1.ParameterObject[]),
          ...((operation.parameters || []) as OpenAPIV3_1.ParameterObject[]),
        ];

        const pathParamsTotal = allParams
          .filter((p) => p.in === "path")
          .map((p) => p.name);
        const queryParamsTotal = allParams
          .filter((p) => p.in === "query")
          .map((p) => p.name);

        // Get request body parameters (top-level properties)
        const bodyParamsTotal: string[] = [];
        if (operation.requestBody) {
          const requestBody =
            operation.requestBody as OpenAPIV3_1.RequestBodyObject;
          const jsonContent = requestBody.content?.["application/json"];
          if (jsonContent?.schema) {
            const schema = jsonContent.schema as OpenAPIV3_1.SchemaObject;
            if (schema.properties) {
              bodyParamsTotal.push(...Object.keys(schema.properties));
            }
          }
        }

        // Get all defined status codes
        const statusCodesTotal = Object.keys(operation.responses || {});

        // Count total parameters and status codes
        const totalParams =
          pathParamsTotal.length +
          queryParamsTotal.length +
          bodyParamsTotal.length;
        const totalStatusCodes = statusCodesTotal.filter(
          (code) => code !== "default",
        ).length;

        report.summary.parameters.total += totalParams;
        report.summary.statusCodes.total += totalStatusCodes;

        if (entry) {
          report.summary.endpoints.covered++;

          // Count covered parameters
          const pathParamsCovered = pathParamsTotal.filter((p) =>
            entry.pathParams.has(p),
          );
          const queryParamsCovered = queryParamsTotal.filter((p) =>
            entry.queryParams.has(p),
          );
          const bodyParamsCovered = bodyParamsTotal.filter((p) =>
            entry.bodyParams.has(p),
          );

          report.summary.parameters.covered +=
            pathParamsCovered.length +
            queryParamsCovered.length +
            bodyParamsCovered.length;

          // Count covered status codes
          const statusCodesCovered = statusCodesTotal.filter(
            (code) =>
              code !== "default" && entry.statusCodes.has(parseInt(code, 10)),
          );
          report.summary.statusCodes.covered += statusCodesCovered.length;

          report.endpoints.covered.push({
            method: method.toUpperCase(),
            path: pathPattern,
            operationId: operation.operationId,
            pathParamsCovered,
            pathParamsTotal,
            queryParamsCovered,
            queryParamsTotal,
            bodyParamsCovered,
            bodyParamsTotal,
            statusCodesCovered: Array.from(entry.statusCodes).sort(
              (a, b) => a - b,
            ),
            statusCodesTotal,
            callCount: entry.callCount,
          });
        } else {
          report.endpoints.uncovered.push({
            method: method.toUpperCase(),
            path: pathPattern,
            operationId: operation.operationId,
            pathParamsTotal,
            queryParamsTotal,
            bodyParamsTotal,
            statusCodesTotal,
          });
        }
      }
    }

    // Calculate percentages
    report.summary.endpoints.percentage =
      report.summary.endpoints.total > 0
        ? (report.summary.endpoints.covered / report.summary.endpoints.total) *
          100
        : 0;

    report.summary.parameters.percentage =
      report.summary.parameters.total > 0
        ? (report.summary.parameters.covered /
            report.summary.parameters.total) *
          100
        : 0;

    report.summary.statusCodes.percentage =
      report.summary.statusCodes.total > 0
        ? (report.summary.statusCodes.covered /
            report.summary.statusCodes.total) *
          100
        : 0;

    return report;
  }

  /**
   * Reset all tracked calls (useful between test runs).
   */
  reset(): void {
    this.calls.clear();
  }

  /**
   * Get current call count for debugging.
   */
  getCallCount(): number {
    let total = 0;
    for (const entry of this.calls.values()) {
      total += entry.callCount;
    }
    return total;
  }

  /**
   * Get raw call data for debugging.
   */
  getCalls(): Map<string, EndpointCoverage> {
    return new Map(this.calls);
  }
}

// Export singleton instance
export const coverageTracker = ApiCoverageTracker.getInstance();

// Export static methods for global setup/teardown
export const clearCoverageData = ApiCoverageTracker.clearTempFile;
export const findAllCoverageTempFiles = ApiCoverageTracker.findAllTempFiles;
export const getCoverageTempFilePath = ApiCoverageTracker.getTempFilePath;
export const getCoverageTempFilePattern = ApiCoverageTracker.getTempFilePattern;

// Export flush function for test teardown
export const flushCoverageData = () => coverageTracker.flush();
