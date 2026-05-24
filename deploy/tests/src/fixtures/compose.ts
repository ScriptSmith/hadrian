import {
  DockerComposeEnvironment,
  StartedDockerComposeEnvironment,
  Wait,
  type WaitStrategy,
} from "testcontainers";
import path from "path";
import { fileURLToPath } from "url";
import { waitForHealthy } from "./wait-for";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * A wait strategy that resolves immediately without inspecting or exec-ing into
 * the container. testcontainers applies `Wait.forListeningPorts()` to every
 * compose service that lacks an explicit strategy, and that check execs into the
 * container — which fails with a 409 ("container is restarting") for services
 * that intentionally crash-loop during setup (e.g. the gateway waiting for a
 * Redis cluster that the test forms after `compose up`). Use this to opt such a
 * service out of startup waiting; wait for its readiness explicitly in the test.
 */
class NoWaitStrategy implements WaitStrategy {
  private startupTimeout = 60000;
  async waitUntilReady(): Promise<void> {}
  withStartupTimeout(startupTimeout: number): WaitStrategy {
    this.startupTimeout = startupTimeout;
    return this;
  }
  isStartupTimeoutSet(): boolean {
    return false;
  }
  getStartupTimeout(): number {
    return this.startupTimeout;
  }
}

export interface ServiceHealthCheck {
  port: number;
  path?: string;
}

export interface ServiceWaitStrategy {
  /**
   * - `http` — wait for an HTTP endpoint to return 2xx
   * - `healthcheck` — wait for the container's Docker healthcheck
   * - `none` — don't wait at all (opt out of the default port-listening probe,
   *   e.g. for services that crash-loop until the test finishes provisioning)
   */
  type: "http" | "healthcheck" | "none";
  port?: number;
  path?: string;
}

export interface ComposeEnvironmentOptions {
  projectName: string;
  composeFile: string;
  waitForServices?: Record<string, ServiceHealthCheck>;
  /** Custom wait strategies for specific services (overrides default port waiting) */
  serviceWaitStrategies?: Record<string, ServiceWaitStrategy>;
  env?: Record<string, string>;
  /** Startup timeout in milliseconds (default: 60000) */
  startupTimeout?: number;
}

export interface ExecResult {
  output: string;
  exitCode: number;
}

export interface StartedComposeEnvironment {
  environment: StartedDockerComposeEnvironment;
  getServiceUrl: (serviceName: string, port: number) => string;
  execInService: (serviceName: string, command: string[]) => Promise<ExecResult>;
  /** Restart a service container (e.g., after config changes) */
  restartService: (serviceName: string) => Promise<void>;
  stop: () => Promise<void>;
}

export async function startComposeEnvironment(
  opts: ComposeEnvironmentOptions
): Promise<StartedComposeEnvironment> {
  // Docker compose files are in deploy/ directory (3 levels up from fixtures/)
  const composeDir = path.join(__dirname, "../../../");
  const composeFilePath = path.join(composeDir, opts.composeFile);

  let compose = new DockerComposeEnvironment(
    path.dirname(composeFilePath),
    path.basename(composeFilePath)
  ).withProjectName(opts.projectName);

  // Apply environment variables
  if (opts.env) {
    for (const [key, value] of Object.entries(opts.env)) {
      compose = compose.withEnvironment({ [key]: value });
    }
  }

  // Apply custom wait strategies for specific services
  // This overrides the default "wait for all exposed ports" behavior
  if (opts.serviceWaitStrategies) {
    for (const [service, strategy] of Object.entries(
      opts.serviceWaitStrategies
    )) {
      if (strategy.type === "http" && strategy.port) {
        compose = compose.withWaitStrategy(
          service,
          Wait.forHttp(strategy.path || "/", strategy.port)
        );
      } else if (strategy.type === "healthcheck") {
        // Use Docker's built-in health check instead of port waiting
        compose = compose.withWaitStrategy(service, Wait.forHealthCheck());
      } else if (strategy.type === "none") {
        // Opt out of testcontainers' default port-listening probe (which execs
        // into the container) for services that crash-loop during setup.
        compose = compose.withWaitStrategy(service, new NoWaitStrategy());
      }
    }
  }

  // Apply startup timeout if specified
  if (opts.startupTimeout) {
    compose = compose.withStartupTimeout(opts.startupTimeout);
  }

  // Skip Docker build if SKIP_BUILD is set (useful when using a pre-built image)
  if (!process.env.SKIP_BUILD) {
    compose = compose.withBuild();
  }

  const environment = await compose.up();

  // Helper function to get a container from the environment
  const getServiceContainer = (serviceName: string) => {
    // Try different naming patterns used by testcontainers/docker compose
    const patterns = [
      `${opts.projectName}-${serviceName}-1`,
      `${opts.projectName}_${serviceName}_1`,
      `${serviceName}-1`,
      serviceName,
    ];

    for (const pattern of patterns) {
      try {
        return environment.getContainer(pattern);
      } catch {
        // Try next pattern
      }
    }

    throw new Error(
      `Could not find container for service "${serviceName}" with project "${opts.projectName}"`
    );
  };

  // Wait for services to be healthy
  if (opts.waitForServices) {
    for (const [service, config] of Object.entries(opts.waitForServices)) {
      const container = getServiceContainer(service);
      const host = container.getHost();
      const port = container.getMappedPort(config.port);
      const healthPath = config.path || "/health";

      await waitForHealthy(`http://${host}:${port}${healthPath}`, {
        maxRetries: 60,
        retryInterval: 2000,
      });
    }
  }

  return {
    environment,
    getServiceUrl: (serviceName: string, port: number) => {
      const container = getServiceContainer(serviceName);
      return `http://${container.getHost()}:${container.getMappedPort(port)}`;
    },
    execInService: async (
      serviceName: string,
      command: string[]
    ): Promise<ExecResult> => {
      const container = getServiceContainer(serviceName);
      const result = await container.exec(command);
      return {
        output: result.output,
        exitCode: result.exitCode,
      };
    },
    /**
     * Restart a service container.
     * Useful for reloading configuration changes (e.g., after SSO config creation).
     */
    restartService: async (serviceName: string): Promise<void> => {
      const container = getServiceContainer(serviceName);
      await container.restart();
    },
    stop: async () => {
      await environment.down({ removeVolumes: true });
    },
  };
}
