export interface WaitForOptions {
  maxRetries?: number;
  retryInterval?: number;
  timeout?: number;
}

export async function waitForHealthy(
  url: string,
  options: WaitForOptions = {}
): Promise<void> {
  const { maxRetries = 30, retryInterval = 1000, timeout = 5000 } = options;

  for (let i = 0; i < maxRetries; i++) {
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeout);

      const response = await fetch(url, {
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (response.ok) {
        return;
      }
    } catch {
      // Connection refused or timeout, continue retrying
    }

    await sleep(retryInterval);
  }

  throw new Error(
    `Service at ${url} did not become healthy after ${maxRetries} retries`
  );
}

export async function waitForPort(
  host: string,
  port: number,
  options: WaitForOptions = {}
): Promise<void> {
  const { maxRetries = 30, retryInterval = 1000 } = options;

  for (let i = 0; i < maxRetries; i++) {
    try {
      const response = await fetch(`http://${host}:${port}`, {
        method: "HEAD",
      });
      if (response) {
        return;
      }
    } catch {
      // Connection refused, continue retrying
    }

    await sleep(retryInterval);
  }

  throw new Error(
    `Port ${port} on ${host} did not become available after ${maxRetries} retries`
  );
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
