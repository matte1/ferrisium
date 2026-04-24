import { test as base, expect } from "@playwright/test";
import { createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";

const renderWaitMs = Number(process.env.FERRISIUM_WEB_TEST_RENDER_WAIT_MS ?? "6500");
const screenshotMinBytes = Number(process.env.FERRISIUM_WEB_TEST_MIN_SCREENSHOT_BYTES ?? "35000");
const bridgeTimeoutMs = Number(process.env.FERRISIUM_WEB_TEST_BRIDGE_TIMEOUT_MS ?? "15000");

// A small 2x2 PNG with distinct opaque colors. The browser demo can request
// remote raster tiles; tests stub those requests so smoke coverage does not
// depend on third-party tile hosts or network access.
const stubTilePng = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nAXBAQEAAACAEP9PF0JQGR7vBPykAXT9AAAAAElFTkSuQmCC",
  "base64",
);

export const test = base.extend({
  diagnostics: async ({ page }, use, testInfo) => {
    const diagnostics = collectBrowserDiagnostics(page);
    await stubExternalRasterRequests(page);
    await use(diagnostics);
    await diagnostics.attach(testInfo);
    diagnostics.expectClean();
  },
});

export { expect };

export async function openDemo(page, path = "/?no_anise=1", options = {}) {
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await page.locator("#bevy").waitFor({ state: "visible" });
  await waitForTestBridge(page, { mode: options.expectedMode });
  await page.waitForTimeout(renderWaitMs);
}

export async function captureRenderSignal(page, testInfo, label) {
  const screenshot = await page.screenshot({ fullPage: false });
  const path = testInfo.outputPath(`${label}.png`);
  await writeFile(path, screenshot);
  await testInfo.attach(`${label}.png`, {
    contentType: "image/png",
    path,
  });

  const pageMetrics = await collectPageMetrics(page);
  const signal = {
    bytes: screenshot.byteLength,
    canvas: pageMetrics.canvas,
    hash: createHash("sha256").update(screenshot).digest("hex"),
    pageMetrics,
  };
  const jsonPath = testInfo.outputPath(`${label}.json`);
  await writeFile(jsonPath, JSON.stringify(signal, null, 2));
  await testInfo.attach(`${label}.json`, {
    contentType: "application/json",
    path: jsonPath,
  });
  return signal;
}

export async function readTestBridge(page) {
  return page.evaluate(() => window.__FERRISIUM_TEST__ ?? null);
}

export async function waitForTestBridge(page, options = {}) {
  const minFrame = options.minFrame ?? 2;
  await page.waitForFunction(
    ({ mode, minFrame }) => {
      const bridge = window.__FERRISIUM_TEST__;
      return Boolean(
        bridge?.interactive &&
          bridge.frame >= minFrame &&
          (mode === undefined || bridge.mode === mode),
      );
    },
    { mode: options.mode, minFrame },
    { timeout: options.timeout ?? bridgeTimeoutMs },
  );
  return readTestBridge(page);
}

export function expectBridgeMode(bridge, mode) {
  expect(bridge, "Ferrisium test bridge was not published").toBeTruthy();
  expect(bridge.mode, "Ferrisium demo mode").toBe(mode);
  expect(bridge.interactive, "Ferrisium demo should be interactive").toBeTruthy();
}

export async function canvasGeometry(page) {
  const box = await page.locator("#bevy").boundingBox();
  expect(box, "Bevy canvas should have a layout box").toBeTruthy();
  return {
    bottom: box.y + box.height,
    center: {
      x: box.x + box.width / 2,
      y: box.y + box.height / 2,
    },
    height: box.height,
    left: box.x,
    right: box.x + box.width,
    top: box.y,
    width: box.width,
  };
}

export const gestures = {
  async dragCanvas(page, options = {}) {
    const canvas = await canvasGeometry(page);
    const from = canvasPoint(canvas, options.from ?? { x: 0.5, y: 0.5 });
    const to = canvasPoint(canvas, options.to ?? { x: 0.65, y: 0.55 });
    await page.mouse.move(from.x, from.y);
    await page.mouse.down({ button: options.button ?? "left" });
    await page.mouse.move(to.x, to.y, { steps: options.steps ?? 18 });
    await page.mouse.up({ button: options.button ?? "left" });
  },
  async wheelCanvas(page, deltaY, options = {}) {
    const canvas = await canvasGeometry(page);
    const point = canvasPoint(canvas, options.at ?? { x: 0.5, y: 0.5 });
    await page.mouse.move(point.x, point.y);
    await page.mouse.wheel(options.deltaX ?? 0, deltaY);
  },
};

export async function recordScenario(page, testInfo, options) {
  await openDemo(page, options.path, { expectedMode: options.expectedMode });
  const bridgeBefore = await readTestBridge(page);
  if (options.expectedMode) {
    expectBridgeMode(bridgeBefore, options.expectedMode);
  }

  const before = await captureRenderSignal(page, testInfo, `${options.label}-before`);
  expectRendered(before, `${options.label} before interaction`);

  const canvas = await canvasGeometry(page);
  await options.steps({ canvas, page, testInfo });

  const targetFrame = Number(bridgeBefore?.frame ?? 0) + (options.minFrameAdvance ?? 2);
  await waitForTestBridge(page, {
    mode: options.expectedMode,
    minFrame: targetFrame,
  });
  await page.waitForTimeout(options.settleMs ?? 1200);

  const after = await captureRenderSignal(page, testInfo, `${options.label}-after`);
  expectRendered(after, `${options.label} after interaction`);
  expect(after.hash, `${options.label} screenshot should change after interaction`).not.toBe(before.hash);
  await attachScenarioSummary(testInfo, options.label, before, after);

  return { after, before };
}

export function expectRendered(signal, label) {
  expect(signal.bytes, `${label} screenshot is too small; the WebGL scene may be blank`).toBeGreaterThan(
    screenshotMinBytes,
  );
  expect(signal.canvas, `${label} did not expose the Bevy canvas`).toBeTruthy();
  expect(signal.canvas.width, `${label} canvas width`).toBeGreaterThan(100);
  expect(signal.canvas.height, `${label} canvas height`).toBeGreaterThan(100);
}

export async function expectInteractionChangesFrame(page, testInfo, label, interact) {
  const before = await captureRenderSignal(page, testInfo, `${label}-before`);
  expectRendered(before, `${label} before interaction`);

  await interact();
  await page.waitForTimeout(1200);

  const after = await captureRenderSignal(page, testInfo, `${label}-after`);
  expectRendered(after, `${label} after interaction`);
  expect(after.hash, `${label} screenshot should change after interaction`).not.toBe(before.hash);
}

async function stubExternalRasterRequests(page) {
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.hostname === "127.0.0.1" || url.hostname === "localhost") {
      await route.continue();
      return;
    }

    await route.fulfill({
      body: stubTilePng,
      contentType: "image/png",
      headers: {
        "Access-Control-Allow-Origin": "*",
        "Cache-Control": "no-store",
      },
      status: 200,
    });
  });
}

function collectBrowserDiagnostics(page) {
  const events = [];
  page.on("console", (message) => {
    events.push({
      location: message.location(),
      text: message.text(),
      type: message.type(),
    });
  });
  page.on("pageerror", (error) => {
    events.push({
      message: error.message,
      stack: error.stack,
      type: "pageerror",
    });
  });
  page.on("requestfailed", (request) => {
    const url = new URL(request.url());
    if (url.hostname !== "127.0.0.1" && url.hostname !== "localhost") {
      return;
    }
    events.push({
      failure: request.failure()?.errorText,
      method: request.method(),
      type: "requestfailed",
      url: request.url(),
    });
  });

  return {
    async attach(testInfo) {
      await testInfo.attach("browser-diagnostics.json", {
        body: JSON.stringify(events, null, 2),
        contentType: "application/json",
      });
    },
    expectClean() {
      const failures = events.filter(
        (event) => event.type === "error" || event.type === "pageerror" || event.type === "requestfailed",
      );
      expect(failures, "browser console/page/request failures").toEqual([]);
    },
  };
}

async function collectPageMetrics(page) {
  return page.evaluate(() => {
    const canvas = document.getElementById("bevy");
    const canvasRect = canvas?.getBoundingClientRect();
    const hud = document.querySelector(".hud");
    return {
      canvas: canvasRect
        ? {
            attrHeight: canvas.getAttribute("height"),
            attrWidth: canvas.getAttribute("width"),
            background: getComputedStyle(canvas).backgroundColor,
            clientHeight: canvas.clientHeight,
            clientWidth: canvas.clientWidth,
            height: canvasRect.height,
            width: canvasRect.width,
          }
        : null,
      devicePixelRatio: window.devicePixelRatio,
      hud: hud
        ? {
            display: getComputedStyle(hud).display,
            hidden: hud.hidden,
          }
        : null,
      viewport: {
        innerHeight: window.innerHeight,
        innerWidth: window.innerWidth,
      },
      testBridge: window.__FERRISIUM_TEST__ ?? null,
    };
  });
}

function canvasPoint(canvas, normalized) {
  return {
    x: canvas.left + canvas.width * normalized.x,
    y: canvas.top + canvas.height * normalized.y,
  };
}

async function attachScenarioSummary(testInfo, label, before, after) {
  const summary = {
    after: scenarioSignalSummary(after),
    before: scenarioSignalSummary(before),
    perfMode: Boolean(process.env.FERRISIUM_WEB_TEST_PERF),
  };
  const path = testInfo.outputPath(`${label}-summary.json`);
  await writeFile(path, JSON.stringify(summary, null, 2));
  await testInfo.attach(`${label}-summary.json`, {
    contentType: "application/json",
    path,
  });
}

function scenarioSignalSummary(signal) {
  const bridge = signal.pageMetrics.testBridge;
  return {
    hash: signal.hash,
    mode: bridge?.mode ?? null,
    frame: bridge?.frame ?? null,
    timing: bridge?.timing ?? null,
    map: bridge?.map ?? null,
    globe: bridge?.globe ?? null,
    metricCamera: bridge?.metricCamera ?? null,
  };
}
