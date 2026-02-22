/**
 * API Coverage Reporter for Vitest
 *
 * Generates a coverage report at the end of the test run showing
 * which API endpoints were exercised.
 *
 * Coverage Thresholds:
 * Set environment variables to enforce minimum coverage:
 * - API_COVERAGE_ENDPOINTS_THRESHOLD: Minimum endpoint coverage % (default: 0)
 * - API_COVERAGE_PARAMETERS_THRESHOLD: Minimum parameter coverage % (default: 0)
 * - API_COVERAGE_STATUS_CODES_THRESHOLD: Minimum status code coverage % (default: 0)
 *
 * If any threshold is set to a value > 0 and coverage falls below it,
 * the test run will fail with a non-zero exit code.
 */
import type { Reporter, File } from "vitest";
import {
  coverageTracker,
  clearCoverageData,
  flushCoverageData,
  type CoverageReport,
} from "../utils/coverage-tracker";
import * as fs from "node:fs/promises";
import * as path from "node:path";

const OPENAPI_SPEC_PATH = path.resolve(
  process.cwd(),
  "../../openapi/hadrian.openapi.json",
);
const COVERAGE_OUTPUT_DIR = path.resolve(process.cwd(), "coverage");
const COVERAGE_OUTPUT_FILE = path.join(
  COVERAGE_OUTPUT_DIR,
  "api-coverage.json",
);

/** Coverage threshold configuration */
interface CoverageThresholds {
  endpoints: number;
  parameters: number;
  statusCodes: number;
}

/** Read coverage thresholds from environment variables */
function getThresholds(): CoverageThresholds {
  return {
    endpoints: parseFloat(
      process.env.API_COVERAGE_ENDPOINTS_THRESHOLD || "0",
    ),
    parameters: parseFloat(
      process.env.API_COVERAGE_PARAMETERS_THRESHOLD || "0",
    ),
    statusCodes: parseFloat(
      process.env.API_COVERAGE_STATUS_CODES_THRESHOLD || "0",
    ),
  };
}

/** Threshold violation info */
interface ThresholdViolation {
  metric: string;
  actual: number;
  threshold: number;
}

export default class ApiCoverageReporter implements Reporter {
  private startTime: number = 0;
  private testCount: number = 0;
  private passedCount: number = 0;
  private failedCount: number = 0;

  async onInit(): Promise<void> {
    this.startTime = Date.now();
    // Ensure coverage directory exists before workers start writing
    await fs.mkdir(COVERAGE_OUTPUT_DIR, { recursive: true });
    // Clear any previous coverage data at the start of the test run
    await clearCoverageData();
  }

  onCollected(files?: File[]): void {
    if (files) {
      this.testCount = files.reduce(
        (count, file) => count + (file.tasks?.length || 0),
        0,
      );
    }
  }

  onFinished(files?: File[]): Promise<void> {
    return this.generateReport(files);
  }

  private async generateReport(files?: File[]): Promise<void> {
    // Count passed/failed tests
    if (files) {
      for (const file of files) {
        for (const task of file.tasks || []) {
          if (task.result?.state === "pass") {
            this.passedCount++;
          } else if (task.result?.state === "fail") {
            this.failedCount++;
          }
        }
      }
    }

    // Flush any pending coverage data before generating the report
    flushCoverageData();

    // Load OpenAPI spec and generate report
    let report: CoverageReport;
    try {
      report = await coverageTracker.generateReport(OPENAPI_SPEC_PATH);
    } catch (error) {
      console.error("\n[API Coverage] Failed to generate report:", error);
      return;
    }

    // Print summary to console
    this.printSummary(report);

    // Write detailed report to file
    await this.writeReportToFile(report);

    // Check coverage thresholds and fail if below
    const violations = this.checkThresholds(report);
    if (violations.length > 0) {
      this.failOnThresholdViolations(violations);
    }
  }

  private printSummary(report: CoverageReport): void {
    const { summary, endpoints, loading } = report;
    const duration = ((Date.now() - this.startTime) / 1000).toFixed(1);

    console.log("\n" + "═".repeat(70));
    console.log("  API COVERAGE REPORT");
    console.log("═".repeat(70));

    // Data loading info
    console.log("\n  Data Collection:");
    console.log(`    Worker files loaded:  ${loading.workerFilesLoaded}`);
    console.log(`    Records loaded:       ${loading.recordsLoaded}`);

    // Show any load errors
    if (loading.loadErrors.length > 0) {
      console.log(`\n  ⚠️  Loading Errors (${loading.loadErrors.length}):`);
      for (const error of loading.loadErrors.slice(0, 5)) {
        console.log(`    - ${error}`);
      }
      if (loading.loadErrors.length > 5) {
        console.log(`    ... and ${loading.loadErrors.length - 5} more`);
      }
    }

    // Warn if no coverage data was collected
    if (loading.workerFilesLoaded === 0) {
      console.log("\n  ⚠️  WARNING: No coverage data files found!");
      console.log(
        "    This may indicate tests didn't run or coverage tracking failed.",
      );
    } else if (loading.recordsLoaded === 0) {
      console.log(
        "\n  ⚠️  WARNING: Coverage files found but no records loaded!",
      );
      console.log("    This may indicate file corruption or parsing errors.");
    }

    // Overall metrics
    console.log("\n  Summary:");
    console.log(`    Endpoints:    ${this.formatCoverage(summary.endpoints)}`);
    console.log(`    Parameters:   ${this.formatCoverage(summary.parameters)}`);
    console.log(
      `    Status Codes: ${this.formatCoverage(summary.statusCodes)}`,
    );

    // API calls tracked
    const totalCalls = coverageTracker.getCallCount();
    console.log(`\n  API Calls Tracked: ${totalCalls}`);

    // Show uncovered endpoints (limited)
    if (endpoints.uncovered.length > 0) {
      console.log(
        `\n  Uncovered Endpoints (${endpoints.uncovered.length} total):`,
      );
      const toShow = endpoints.uncovered.slice(0, 15);
      for (const endpoint of toShow) {
        console.log(`    - ${endpoint.method} ${endpoint.path}`);
      }
      if (endpoints.uncovered.length > 15) {
        console.log(
          `    ... and ${endpoints.uncovered.length - 15} more (see coverage/api-coverage.json)`,
        );
      }
    }

    // Test run stats
    console.log(`\n  Test Duration: ${duration}s`);

    // Show configured thresholds if any are set
    const thresholds = getThresholds();
    const hasThresholds =
      thresholds.endpoints > 0 ||
      thresholds.parameters > 0 ||
      thresholds.statusCodes > 0;

    if (hasThresholds) {
      console.log("\n  Configured Thresholds:");
      if (thresholds.endpoints > 0) {
        const status =
          summary.endpoints.percentage >= thresholds.endpoints ? "✓" : "✗";
        console.log(`    ${status} Endpoints:    ${thresholds.endpoints}%`);
      }
      if (thresholds.parameters > 0) {
        const status =
          summary.parameters.percentage >= thresholds.parameters ? "✓" : "✗";
        console.log(`    ${status} Parameters:   ${thresholds.parameters}%`);
      }
      if (thresholds.statusCodes > 0) {
        const status =
          summary.statusCodes.percentage >= thresholds.statusCodes ? "✓" : "✗";
        console.log(`    ${status} Status Codes: ${thresholds.statusCodes}%`);
      }
    }

    console.log("═".repeat(70) + "\n");
  }

  private formatCoverage(metric: {
    covered: number;
    total: number;
    percentage: number;
  }): string {
    const bar = this.createProgressBar(metric.percentage, 20);
    return `${metric.covered.toString().padStart(3)}/${metric.total.toString().padEnd(3)} ${bar} ${metric.percentage.toFixed(1)}%`;
  }

  private createProgressBar(percentage: number, width: number): string {
    const filled = Math.round((percentage / 100) * width);
    const empty = width - filled;
    return `[${"\u2588".repeat(filled)}${"\u2591".repeat(empty)}]`;
  }

  private async writeReportToFile(report: CoverageReport): Promise<void> {
    try {
      // Ensure coverage directory exists
      await fs.mkdir(COVERAGE_OUTPUT_DIR, { recursive: true });

      // Write JSON report
      await fs.writeFile(
        COVERAGE_OUTPUT_FILE,
        JSON.stringify(report, null, 2),
        "utf-8",
      );

      console.log(`  Coverage report written to: ${COVERAGE_OUTPUT_FILE}\n`);
    } catch (error) {
      console.error("\n[API Coverage] Failed to write report:", error);
    }
  }

  /**
   * Check coverage against configured thresholds.
   * Returns list of violations if any thresholds are not met.
   */
  private checkThresholds(report: CoverageReport): ThresholdViolation[] {
    const thresholds = getThresholds();
    const violations: ThresholdViolation[] = [];

    // Check endpoints threshold
    if (
      thresholds.endpoints > 0 &&
      report.summary.endpoints.percentage < thresholds.endpoints
    ) {
      violations.push({
        metric: "Endpoints",
        actual: report.summary.endpoints.percentage,
        threshold: thresholds.endpoints,
      });
    }

    // Check parameters threshold
    if (
      thresholds.parameters > 0 &&
      report.summary.parameters.percentage < thresholds.parameters
    ) {
      violations.push({
        metric: "Parameters",
        actual: report.summary.parameters.percentage,
        threshold: thresholds.parameters,
      });
    }

    // Check status codes threshold
    if (
      thresholds.statusCodes > 0 &&
      report.summary.statusCodes.percentage < thresholds.statusCodes
    ) {
      violations.push({
        metric: "Status Codes",
        actual: report.summary.statusCodes.percentage,
        threshold: thresholds.statusCodes,
      });
    }

    return violations;
  }

  /**
   * Print threshold violations and throw to fail the test run.
   */
  private failOnThresholdViolations(violations: ThresholdViolation[]): void {
    console.log("\n" + "!".repeat(70));
    console.log("  API COVERAGE THRESHOLD VIOLATIONS");
    console.log("!".repeat(70));

    for (const violation of violations) {
      console.log(
        `\n  ❌ ${violation.metric}: ${violation.actual.toFixed(1)}% < ${violation.threshold}% (threshold)`,
      );
    }

    console.log("\n" + "!".repeat(70));
    console.log(
      "  Set thresholds via environment variables:",
    );
    console.log("    API_COVERAGE_ENDPOINTS_THRESHOLD");
    console.log("    API_COVERAGE_PARAMETERS_THRESHOLD");
    console.log("    API_COVERAGE_STATUS_CODES_THRESHOLD");
    console.log("!".repeat(70) + "\n");

    // Throw error to fail the test run
    throw new Error(
      `API coverage below threshold: ${violations.map((v) => `${v.metric} (${v.actual.toFixed(1)}% < ${v.threshold}%)`).join(", ")}`,
    );
  }
}
