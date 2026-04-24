import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(new URL(".", import.meta.url).pathname);
const host = process.env.FERRISIUM_WEB_TEST_HOST ?? "127.0.0.1";
const port = Number(process.env.FERRISIUM_WEB_TEST_PORT ?? "8083");
const baseURL = `http://${host}:${port}`;
const perfEnabled = Boolean(process.env.FERRISIUM_WEB_TEST_PERF);
const scenariosEnabled = Boolean(process.env.FERRISIUM_WEB_TEST_SCENARIOS);
const testTimeoutMs = Number(
  process.env.FERRISIUM_WEB_TEST_TIMEOUT_MS ?? (scenariosEnabled ? "300000" : "45000"),
);
const videoMode = process.env.FERRISIUM_WEB_TEST_VIDEO ?? (process.env.CI ? "off" : "retain-on-failure");
const chromeExecutable = [
  process.env.PLAYWRIGHT_CHROME,
  process.env.CHROME,
  "/usr/bin/google-chrome",
  "/usr/bin/chromium-browser",
].find((candidate) => candidate && existsSync(candidate));

export default defineConfig({
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false,
  outputDir: perfEnabled ? "target/playwright-perf-results" : "target/playwright-results",
  projects: [
    {
      name: "chromium",
      use: {
        baseURL,
        browserName: "chromium",
        launchOptions: {
          ...(chromeExecutable ? { executablePath: chromeExecutable } : {}),
          args: [
            "--enable-webgl",
            "--enable-unsafe-swiftshader",
            "--ignore-gpu-blocklist",
            "--no-sandbox",
          ],
        },
        screenshot: "only-on-failure",
        trace: perfEnabled ? "off" : "retain-on-failure",
        viewport: { width: 1024, height: 768 },
        video: videoMode,
      },
    },
  ],
  reporter: [
    ["list"],
    [
      "html",
      {
        open: "never",
        outputFolder: perfEnabled ? "target/playwright-perf-report" : "target/playwright-report",
      },
    ],
  ],
  testDir: "tests/browser",
  testMatch: scenariosEnabled ? ["**/*.scenario.spec.mjs"] : ["**/*.spec.mjs"],
  testIgnore: scenariosEnabled ? [] : ["**/*.scenario.spec.mjs"],
  timeout: testTimeoutMs,
  workers: scenariosEnabled ? 1 : undefined,
  webServer: {
    command: `node scripts/static_no_cache_server.mjs --directory ${resolve(
      repoRoot,
      "examples/ferrisium_demo/dist",
    )} --host ${host} --port ${port}`,
    reuseExistingServer: !process.env.CI,
    stderr: "pipe",
    stdout: "pipe",
    timeout: 10_000,
    url: baseURL,
  },
});
