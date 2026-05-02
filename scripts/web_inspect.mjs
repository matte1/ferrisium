#!/usr/bin/env node

import { spawn } from "node:child_process";
import { createReadStream, existsSync, statSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize, resolve, sep } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { inflateSync } from "node:zlib";

const repoRoot = resolve(new URL("..", import.meta.url).pathname);
const defaults = {
  chrome: process.env.CHROME ?? "google-chrome",
  debugPort: 9222,
  dist: join(repoRoot, "examples/ferrisium_demo/dist"),
  headless: true,
  host: "127.0.0.1",
  keepOpen: false,
  out: join(repoRoot, "target/web-inspect/latest"),
  path: "/",
  port: 8082,
  scenario: "globe-wheel",
  waitMs: 8_000,
};
const h3GlobeInspectPath =
  "/?h3_inspect=1&globe_lon=-98&globe_lat=39&globe_distance_factor=1.6";
const solarEarthCloseInspectPath =
  "/?view=solar&solar_focus=earth&trail_months=1";

const mimeTypes = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".jpeg", "image/jpeg"],
  [".wasm", "application/wasm"],
  [".css", "text/css; charset=utf-8"],
  [".ico", "image/x-icon"],
]);

const usage = `Usage: node scripts/web_inspect.mjs [options]

Build the demo first with \`just web-build\`, then run this script or use
\`just web-inspect\`.

Options:
  --chrome <path>       Chrome executable. Default: ${defaults.chrome}
  --debug-port <port>   Chrome DevTools Protocol port. Default: ${defaults.debugPort}
  --dist <path>         Directory to serve. Default: ${relativePath(defaults.dist)}
  --headed              Open a visible browser window instead of headless Chrome.
  --host <host>         Static server host. Default: ${defaults.host}
  --keep-open           Leave Chrome and the server running after capture.
  --out <path>          Artifact directory. Default: ${relativePath(defaults.out)}
  --path <path>         URL path/query to inspect. Default: ${defaults.path}; h3-globe-inspect uses ${h3GlobeInspectPath}; solar-earth-close-inspect uses ${solarEarthCloseInspectPath}
  --port <port>         Static server port. Default: ${defaults.port}
  --scenario <name>     globe-wheel, globe-drag, globe-pan, globe-touch-pinch, h3-globe-inspect, map-wheel, focus-mars-shortcut, solar-drag, solar-wheel, solar-slider, solar-earth-close-inspect, or load. Default: ${defaults.scenario}
  --wait-ms <ms>        Initial render wait before input. Default: ${defaults.waitMs}
  --help                Show this help.
`;

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : error);
  process.exitCode = 1;
});

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log(usage);
    return;
  }

  if (!existsSync(join(options.dist, "index.html"))) {
    throw new Error(
      `missing ${join(options.dist, "index.html")}; run \`just web-build\` first`,
    );
  }

  await mkdir(options.out, { recursive: true });

  const server = await startStaticServer(options);
  const chrome = launchChrome(options, server.url);
  const cdp = await connectToPage(options);

  const artifacts = [];
  const events = [];
  cdp.onEvent((event) => events.push(event));

  try {
    await cdp.send("Page.enable");
    await cdp.send("Runtime.enable");
    await cdp.send("Log.enable");

    let renderError;
    await sleep(options.waitMs);
    artifacts.push(await capture(cdp, options.out, "initial"));
    try {
      await assertRendered(artifacts.at(-1), options.scenario);
    } catch (error) {
      renderError = error;
    }

    if (!renderError) {
      await runScenario(cdp, options.scenario);
    }
    if (!renderError && options.scenario !== "load") {
      await sleep(2_500);
      artifacts.push(await capture(cdp, options.out, "after-input"));
      try {
        await assertRendered(artifacts.at(-1), options.scenario);
      } catch (error) {
        renderError = error;
      }
    }

    const pageMetrics = await collectPageMetrics(cdp);
    const summary = {
      artifacts,
      events,
      pageMetrics,
      renderError: renderError instanceof Error ? renderError.message : undefined,
      scenario: options.scenario,
      url: server.url,
    };
    await writeFile(
      join(options.out, "summary.json"),
      `${JSON.stringify(summary, null, 2)}\n`,
    );

    console.log(`web inspection complete: ${relativePath(options.out)}`);
    for (const artifact of artifacts) {
      console.log(`  ${relativePath(artifact.path)} (${artifact.bytes} bytes)`);
    }
    if (events.length > 0) {
      console.log(`  captured ${events.length} browser log event(s)`);
    }
    if (renderError) {
      throw renderError;
    }

    if (options.keepOpen) {
      console.log("keeping Chrome and the static server open; press Ctrl+C to stop");
      await new Promise(() => {});
    }
  } finally {
    if (!options.keepOpen) {
      await cdp.closeBrowser().catch(() => undefined);
      chrome.kill("SIGTERM");
      await closeServer(server.instance);
    }
  }
}

function parseArgs(args) {
  const options = { ...defaults };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    switch (arg) {
      case "--":
        break;
      case "--chrome":
        options.chrome = requiredValue(args, ++index, arg);
        break;
      case "--debug-port":
        options.debugPort = numberValue(args, ++index, arg);
        break;
      case "--dist":
        options.dist = resolve(requiredValue(args, ++index, arg));
        break;
      case "--headed":
        options.headless = false;
        break;
      case "--help":
        options.help = true;
        break;
      case "--host":
        options.host = requiredValue(args, ++index, arg);
        break;
      case "--keep-open":
        options.keepOpen = true;
        break;
      case "--out":
        options.out = resolve(requiredValue(args, ++index, arg));
        break;
      case "--path":
        options.path = requiredValue(args, ++index, arg);
        break;
      case "--port":
        options.port = numberValue(args, ++index, arg);
        break;
      case "--scenario":
        options.scenario = requiredValue(args, ++index, arg);
        break;
      case "--wait-ms":
        options.waitMs = numberValue(args, ++index, arg);
        break;
      default:
        throw new Error(`unknown argument: ${arg}\n${usage}`);
    }
  }

  if (
    ![
      "globe-wheel",
      "globe-drag",
      "globe-pan",
      "globe-touch-pinch",
      "h3-globe-inspect",
      "map-wheel",
      "focus-mars-shortcut",
      "solar-drag",
      "solar-wheel",
      "solar-slider",
      "solar-earth-close-inspect",
      "load",
    ].includes(options.scenario)
  ) {
    throw new Error(`unknown scenario: ${options.scenario}`);
  }
  if (!options.path.startsWith("/")) {
    options.path = `/${options.path}`;
  }
  if (options.scenario === "h3-globe-inspect" && options.path === defaults.path) {
    options.path = h3GlobeInspectPath;
  }
  if (options.scenario === "solar-earth-close-inspect" && options.path === defaults.path) {
    options.path = solarEarthCloseInspectPath;
  }

  return options;
}

function requiredValue(args, index, flag) {
  const value = args[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function numberValue(args, index, flag) {
  const value = Number(requiredValue(args, index, flag));
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${flag} requires a positive integer`);
  }
  return value;
}

async function startStaticServer(options) {
  const root = resolve(options.dist);
  const server = createServer((request, response) => {
    const requestUrl = new URL(request.url ?? "/", `http://${options.host}`);
    if (requestUrl.pathname === "/favicon.ico") {
      response.writeHead(204, { "Cache-Control": "no-store" });
      response.end();
      return;
    }

    const pathname = requestUrl.pathname === "/" ? "/index.html" : requestUrl.pathname;
    const path = normalize(join(root, decodeURIComponent(pathname)));

    if (!path.startsWith(`${root}${sep}`) && path !== root) {
      response.writeHead(403);
      response.end("Forbidden");
      return;
    }
    if (!existsSync(path) || !statSync(path).isFile()) {
      response.writeHead(404);
      response.end("Not found");
      return;
    }

    response.writeHead(200, {
      "Cache-Control": "no-store",
      "Content-Type": mimeTypes.get(extname(path)) ?? "application/octet-stream",
    });
    createReadStream(path).pipe(response);
  });

  await new Promise((resolveStart, rejectStart) => {
    server.once("error", rejectStart);
    server.listen(options.port, options.host, () => {
      server.off("error", rejectStart);
      resolveStart();
    });
  });

  return {
    instance: server,
    url: `http://${options.host}:${options.port}${options.path}`,
  };
}

function launchChrome(options, url) {
  const userDataDir = join("/tmp", `ferrisium-web-inspect-${process.pid}`);
  const args = [
    ...(options.headless ? ["--headless=new"] : []),
    "--no-sandbox",
    "--enable-webgl",
    "--enable-unsafe-swiftshader",
    "--ignore-gpu-blocklist",
    "--window-size=1024,768",
    `--remote-debugging-port=${options.debugPort}`,
    `--user-data-dir=${userDataDir}`,
    url,
  ];

  return spawn(options.chrome, args, {
    stdio: ["ignore", "pipe", "pipe"],
  }).on("error", (error) => {
    throw error;
  });
}

async function connectToPage(options) {
  const deadline = Date.now() + 15_000;
  let target;
  while (Date.now() < deadline) {
    try {
      const targets = await fetch(`http://${options.host}:${options.debugPort}/json`).then(
        (response) => response.json(),
      );
      target = targets.find((candidate) => candidate.type === "page") ?? targets[0];
      if (target?.webSocketDebuggerUrl) {
        break;
      }
    } catch {
      // Chrome may not have opened the debugging endpoint yet.
    }
    await sleep(250);
  }

  if (!target?.webSocketDebuggerUrl) {
    throw new Error("could not connect to Chrome DevTools Protocol");
  }

  return CdpClient.connect(target.webSocketDebuggerUrl);
}

async function runScenario(cdp, scenario) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: 512,
    y: 384,
  });

  switch (scenario) {
    case "globe-wheel":
      await wheel(cdp, 512, 384, -120, 8);
      break;
    case "globe-drag":
      await drag(cdp, 520, 390, 700, 460);
      break;
    case "globe-pan":
      await drag(cdp, 520, 390, 700, 460, "right", 2);
      break;
    case "globe-touch-pinch":
      await touchPinch(cdp, 512, 384);
      break;
    case "h3-globe-inspect":
      break;
    case "map-wheel":
      await wheel(cdp, 512, 384, -120, 6);
      break;
    case "focus-mars-shortcut":
      await key(cdp, "Digit3", "3", 51);
      break;
    case "solar-drag":
      await drag(cdp, 560, 420, 710, 330);
      break;
    case "solar-wheel":
      await wheel(cdp, 640, 420, -120, 5);
      break;
    case "solar-slider":
      await setSolarTrailMonths(cdp, 3);
      break;
    case "solar-earth-close-inspect":
      break;
    case "load":
      break;
    default:
      throw new Error(`unknown scenario: ${scenario}`);
  }
}

async function setSolarTrailMonths(cdp, months) {
  await cdp.send("Runtime.evaluate", {
    expression: `
      (() => {
        const slider = document.getElementById("solar-trail-months");
        slider.value = String(${months});
        slider.dispatchEvent(new InputEvent("input", { bubbles: true }));
      })();
    `,
  });
}

async function wheel(cdp, x, y, deltaY, count) {
  for (let index = 0; index < count; index += 1) {
    await cdp.send("Input.dispatchMouseEvent", {
      deltaX: 0,
      deltaY,
      type: "mouseWheel",
      x,
      y,
    });
    await sleep(80);
  }
}

async function drag(cdp, startX, startY, endX, endY, button = "left", buttons = 1) {
  await cdp.send("Input.dispatchMouseEvent", {
    button,
    clickCount: 1,
    type: "mousePressed",
    x: startX,
    y: startY,
  });

  const steps = 12;
  for (let step = 1; step <= steps; step += 1) {
    await cdp.send("Input.dispatchMouseEvent", {
      button,
      buttons,
      type: "mouseMoved",
      x: startX + ((endX - startX) * step) / steps,
      y: startY + ((endY - startY) * step) / steps,
    });
    await sleep(30);
  }

  await cdp.send("Input.dispatchMouseEvent", {
    button,
    clickCount: 1,
    type: "mouseReleased",
    x: endX,
    y: endY,
  });
}

async function touchPinch(cdp, centerX, centerY) {
  const firstId = 1;
  const secondId = 2;
  await cdp.send("Input.dispatchTouchEvent", {
    touchPoints: [
      touchPoint(firstId, centerX - 34, centerY),
      touchPoint(secondId, centerX + 34, centerY),
    ],
    type: "touchStart",
  });

  const steps = 10;
  for (let step = 1; step <= steps; step += 1) {
    const centerOffsetX = step * 5;
    const distance = 34 + step * 4;
    await cdp.send("Input.dispatchTouchEvent", {
      touchPoints: [
        touchPoint(firstId, centerX + centerOffsetX - distance, centerY + step * 2),
        touchPoint(secondId, centerX + centerOffsetX + distance, centerY + step * 2),
      ],
      type: "touchMove",
    });
    await sleep(45);
  }

  await cdp.send("Input.dispatchTouchEvent", {
    touchPoints: [],
    type: "touchEnd",
  });
}

function touchPoint(id, x, y) {
  return {
    force: 1,
    id,
    radiusX: 8,
    radiusY: 8,
    x,
    y,
  };
}

async function key(cdp, code, keyValue, virtualKeyCode) {
  await cdp.send("Input.dispatchKeyEvent", {
    code,
    key: keyValue,
    nativeVirtualKeyCode: virtualKeyCode,
    text: keyValue,
    type: "keyDown",
    unmodifiedText: keyValue,
    windowsVirtualKeyCode: virtualKeyCode,
  });
  await sleep(30);
  await cdp.send("Input.dispatchKeyEvent", {
    code,
    key: keyValue,
    nativeVirtualKeyCode: virtualKeyCode,
    type: "keyUp",
    windowsVirtualKeyCode: virtualKeyCode,
  });
}

async function capture(cdp, out, label) {
  const result = await cdp.send("Page.captureScreenshot", {
    captureBeyondViewport: false,
    format: "png",
  });
  const path = join(out, `${label}.png`);
  const buffer = Buffer.from(result.data, "base64");
  await writeFile(path, buffer);
  return { bytes: buffer.byteLength, label, path };
}

async function collectPageMetrics(cdp) {
  const result = await cdp.send("Runtime.evaluate", {
    expression: `
      (() => {
        const canvas = document.getElementById("bevy");
        const canvasRect = canvas?.getBoundingClientRect();
        const sourcePanel = document.getElementById("source-panel");
        const hud = document.querySelector(".hud");
        return {
          devicePixelRatio: window.devicePixelRatio,
          documentBackground: getComputedStyle(document.body).backgroundColor,
          viewport: {
            innerWidth: window.innerWidth,
            innerHeight: window.innerHeight,
            visualWidth: window.visualViewport?.width ?? null,
            visualHeight: window.visualViewport?.height ?? null,
          },
          canvas: canvasRect ? {
            width: canvasRect.width,
            height: canvasRect.height,
            clientWidth: canvas.clientWidth,
            clientHeight: canvas.clientHeight,
            attrWidth: canvas.getAttribute("width"),
            attrHeight: canvas.getAttribute("height"),
            background: getComputedStyle(canvas).backgroundColor,
          } : null,
          hud: hud ? {
            hidden: hud.hidden,
            width: hud.getBoundingClientRect().width,
            height: hud.getBoundingClientRect().height,
          } : null,
          sourcePanel: sourcePanel ? {
            hidden: sourcePanel.hidden,
            display: getComputedStyle(sourcePanel).display,
          } : null,
        };
      })();
    `,
    returnByValue: true,
  });

  return result.result?.value ?? null;
}

async function assertRendered(artifact, scenario) {
  // Sparse scenes such as the metric solar-system view compress well even when
  // they have visible WebGL geometry. UI-only blank captures are roughly 26 KB.
  if (artifact.bytes < 40_000) {
    throw new Error(
      `${relativePath(artifact.path)} is only ${artifact.bytes} bytes; the WebGL scene may be blank`,
    );
  }

  if (scenario === "h3-globe-inspect") {
    artifact.h3InspectionPixels = await countH3InspectionPixels(artifact.path);
  }
}

async function countH3InspectionPixels(path) {
  const png = await readFile(path);
  const stats = countPngPixels(png, isH3InspectionPixel);
  if (stats.count < 1_000) {
    throw new Error(
      `${relativePath(path)} has only ${stats.count} high-contrast H3 inspection pixels; the globe H3 overlay may be missing or z-fighting`,
    );
  }
  return stats.count;
}

function isH3InspectionPixel(red, green, blue, alpha) {
  return (
    alpha > 0 &&
    red >= 150 &&
    blue >= 100 &&
    green <= 135 &&
    red > green * 1.35 &&
    blue > green * 1.15
  );
}

function countPngPixels(buffer, predicate) {
  const signature = buffer.subarray(0, 8).toString("hex");
  if (signature !== "89504e470d0a1a0a") {
    throw new Error("capture is not a PNG file");
  }

  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  let interlace = 0;
  const idatChunks = [];
  let offset = 8;
  while (offset < buffer.length) {
    const length = buffer.readUInt32BE(offset);
    offset += 4;
    const type = buffer.toString("ascii", offset, offset + 4);
    offset += 4;
    const data = buffer.subarray(offset, offset + length);
    offset += length + 4;

    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      interlace = data[12];
    } else if (type === "IDAT") {
      idatChunks.push(data);
    } else if (type === "IEND") {
      break;
    }
  }

  if (bitDepth !== 8 || interlace !== 0 || ![2, 6].includes(colorType)) {
    throw new Error(
      `unsupported PNG format: bitDepth=${bitDepth} colorType=${colorType} interlace=${interlace}`,
    );
  }

  const bytesPerPixel = colorType === 6 ? 4 : 3;
  const stride = width * bytesPerPixel;
  const inflated = inflateSync(Buffer.concat(idatChunks));
  let sourceOffset = 0;
  let count = 0;
  let previous = Buffer.alloc(stride);
  let current = Buffer.alloc(stride);

  for (let y = 0; y < height; y += 1) {
    const filter = inflated[sourceOffset];
    sourceOffset += 1;
    for (let x = 0; x < stride; x += 1) {
      const filtered = inflated[sourceOffset + x];
      const left = x >= bytesPerPixel ? current[x - bytesPerPixel] : 0;
      const up = previous[x];
      const upLeft = x >= bytesPerPixel ? previous[x - bytesPerPixel] : 0;
      current[x] = unfilterPngByte(filter, filtered, left, up, upLeft);
    }
    sourceOffset += stride;

    for (let x = 0; x < width; x += 1) {
      const pixel = x * bytesPerPixel;
      const red = current[pixel];
      const green = current[pixel + 1];
      const blue = current[pixel + 2];
      const alpha = bytesPerPixel === 4 ? current[pixel + 3] : 255;
      if (predicate(red, green, blue, alpha, x, y, width, height)) {
        count += 1;
      }
    }

    const nextPrevious = previous;
    previous = current;
    current = nextPrevious;
  }

  return { count, height, width };
}

function unfilterPngByte(filter, value, left, up, upLeft) {
  switch (filter) {
    case 0:
      return value;
    case 1:
      return (value + left) & 0xff;
    case 2:
      return (value + up) & 0xff;
    case 3:
      return (value + Math.floor((left + up) / 2)) & 0xff;
    case 4:
      return (value + paethPredictor(left, up, upLeft)) & 0xff;
    default:
      throw new Error(`unsupported PNG filter: ${filter}`);
  }
}

function paethPredictor(left, up, upLeft) {
  const estimate = left + up - upLeft;
  const leftDistance = Math.abs(estimate - left);
  const upDistance = Math.abs(estimate - up);
  const upLeftDistance = Math.abs(estimate - upLeft);
  if (leftDistance <= upDistance && leftDistance <= upLeftDistance) {
    return left;
  }
  if (upDistance <= upLeftDistance) {
    return up;
  }
  return upLeft;
}

function closeServer(server) {
  return new Promise((resolveClose) => server.close(resolveClose));
}

function relativePath(path) {
  return path.startsWith(repoRoot) ? path.slice(repoRoot.length + 1) : path;
}

class CdpClient {
  static async connect(url) {
    const socket = new WebSocket(url);
    const client = new CdpClient(socket);
    await new Promise((resolveConnect, rejectConnect) => {
      socket.onopen = resolveConnect;
      socket.onerror = rejectConnect;
    });
    return client;
  }

  constructor(socket) {
    this.events = [];
    this.eventSink = () => undefined;
    this.nextId = 0;
    this.pending = new Map();
    this.socket = socket;
    socket.onmessage = (message) => this.handleMessage(message);
  }

  send(method, params = {}) {
    const id = ++this.nextId;
    this.socket.send(JSON.stringify({ id, method, params }));
    return new Promise((resolveSend, rejectSend) => {
      this.pending.set(id, { reject: rejectSend, resolve: resolveSend });
    });
  }

  onEvent(callback) {
    this.eventSink = callback;
  }

  async closeBrowser() {
    await this.send("Browser.close");
  }

  handleMessage(message) {
    const payload = JSON.parse(message.data);
    if (payload.id && this.pending.has(payload.id)) {
      const pending = this.pending.get(payload.id);
      this.pending.delete(payload.id);
      if (payload.error) {
        pending.reject(new Error(JSON.stringify(payload.error)));
      } else {
        pending.resolve(payload.result);
      }
      return;
    }

    if (payload.method === "Runtime.consoleAPICalled") {
      this.eventSink({
        args: payload.params.args.map(formatCdpValue),
        type: payload.params.type,
      });
    }
    if (payload.method === "Runtime.exceptionThrown") {
      this.eventSink({
        exception: payload.params.exceptionDetails.text,
        type: "exception",
      });
    }
    if (payload.method === "Log.entryAdded") {
      const entry = payload.params.entry;
      if (entry.level === "error" || entry.level === "warning") {
        this.eventSink({
          level: entry.level,
          text: truncate(entry.text),
          type: "log",
        });
      }
    }
  }
}

function formatCdpValue(value) {
  return truncate(value.value ?? value.description ?? value.type);
}

function truncate(value) {
  const text = String(value);
  const maxLength = 600;
  if (text.length <= maxLength) {
    return text;
  }

  return `${text.slice(0, maxLength)}...`;
}
