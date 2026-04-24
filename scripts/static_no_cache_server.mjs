#!/usr/bin/env node

import { createReadStream, existsSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize, resolve, sep } from "node:path";

const mimeTypes = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
  [".wasm", "application/wasm"],
]);

const defaults = {
  directory: ".",
  host: "127.0.0.1",
  port: 8081,
};

function usage() {
  console.log(`Usage: node scripts/static_no_cache_server.mjs [options]

Options:
  --directory <path>  Static file root. Default: ${defaults.directory}
  --host <host>       Bind host. Default: ${defaults.host}
  --port <port>       Bind port. Default: ${defaults.port}
  --help              Show this help text
`);
}

function parseArgs(args) {
  const options = { ...defaults };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    switch (arg) {
      case "--directory":
        options.directory = requiredValue(args, ++index, arg);
        break;
      case "--host":
        options.host = requiredValue(args, ++index, arg);
        break;
      case "--port":
        options.port = positiveInteger(requiredValue(args, ++index, arg), arg);
        break;
      case "--help":
      case "-h":
        usage();
        process.exit(0);
        break;
      default:
        throw new Error(`unknown argument: ${arg}`);
    }
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

function positiveInteger(value, flag) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} requires a positive integer`);
  }
  return parsed;
}

function staticPath(root, requestUrl) {
  const pathname = requestUrl.pathname === "/" ? "/index.html" : requestUrl.pathname;
  const path = normalize(join(root, decodeURIComponent(pathname)));
  if (!path.startsWith(`${root}${sep}`) && path !== root) {
    return null;
  }
  return path;
}

function sendError(response, status, message) {
  response.writeHead(status, {
    "Cache-Control": "no-store, max-age=0, must-revalidate",
    "Content-Type": "text/plain; charset=utf-8",
    Expires: "0",
    Pragma: "no-cache",
  });
  response.end(message);
}

function serve(options) {
  const root = resolve(options.directory);
  if (!existsSync(root) || !statSync(root).isDirectory()) {
    throw new Error(`missing static directory: ${root}`);
  }

  const server = createServer((request, response) => {
    const requestUrl = new URL(request.url ?? "/", `http://${options.host}`);
    if (requestUrl.pathname === "/favicon.ico") {
      response.writeHead(204, {
        "Cache-Control": "no-store, max-age=0, must-revalidate",
        Expires: "0",
        Pragma: "no-cache",
      });
      response.end();
      return;
    }

    const path = staticPath(root, requestUrl);
    if (path === null) {
      sendError(response, 403, "Forbidden");
      return;
    }
    if (!existsSync(path) || !statSync(path).isFile()) {
      sendError(response, 404, "Not found");
      return;
    }

    response.writeHead(200, {
      "Cache-Control": "no-store, max-age=0, must-revalidate",
      "Content-Type": mimeTypes.get(extname(path)) ?? "application/octet-stream",
      Expires: "0",
      Pragma: "no-cache",
    });
    createReadStream(path).pipe(response);
  });

  server.listen(options.port, options.host, () => {
    console.log(`serving ${root} at http://${options.host}:${options.port}`);
  });
}

try {
  serve(parseArgs(process.argv.slice(2)));
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
