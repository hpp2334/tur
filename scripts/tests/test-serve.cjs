const { spawn } = require("child_process");
const http = require("http");
const fs = require("fs");
const path = require("path");

const root = path.join(__dirname, "../..");
const cliBin = path.join(root, "js/packages/tur-wasm-cli/bin/cli.cjs");
const pkgDir = path.join(root, "libs/tur-wasm/pkg");
const tmpJs = path.join(root, "scripts/tests/fixtures/test-bundle.js");
const port = 9876;

function fetch(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let body = "";
      res.on("data", (chunk) => (body += chunk));
      res.on("end", () => resolve({ status: res.statusCode, headers: res.headers, body }));
    }).on("error", reject);
  });
}

async function main() {
  fs.mkdirSync(pkgDir, { recursive: true });
  fs.mkdirSync(path.dirname(tmpJs), { recursive: true });
  fs.writeFileSync(tmpJs, 'globalThis.startApp = () => {};');
  fs.writeFileSync(path.join(pkgDir, "tur_wasm.js"), "export function init() {}");
  fs.writeFileSync(path.join(pkgDir, "tur_wasm_bg.wasm"), Buffer.from([0x00, 0x61, 0x73, 0x6d]));

  const child = spawn(process.execPath, [cliBin, "serve", tmpJs, "--port", String(port), "--no-build"], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
  });

  let started = false;
  child.stdout.on("data", (data) => {
    if (data.includes("Serving at")) started = true;
  });

  await new Promise((resolve) => {
    const timeout = setTimeout(() => {
      resolve();
    }, 5000);
    const interval = setInterval(() => {
      if (started) {
        clearInterval(interval);
        clearTimeout(timeout);
        resolve();
      }
    }, 100);
  });

  if (!started) {
    child.kill("SIGTERM");
    console.error("FAIL: server did not start");
    process.exit(1);
  }

  let passed = true;

  const indexRes = await fetch(`http://localhost:${port}/`);
  if (indexRes.status !== 200) {
    console.error(`FAIL: GET / returned ${indexRes.status}, expected 200`);
    passed = false;
  } else {
    console.log(`PASS: GET / returned 200`);
  }
  if (!indexRes.body.includes("tur wasm demo")) {
    console.error("FAIL: index.html missing expected title");
    passed = false;
  } else {
    console.log("PASS: index.html contains expected title");
  }
  if (!indexRes.body.includes("test-bundle.js")) {
    console.error("FAIL: index.html missing JS filename");
    passed = false;
  } else {
    console.log("PASS: index.html references test-bundle.js");
  }

  const jsRes = await fetch(`http://localhost:${port}/test-bundle.js`);
  if (jsRes.status !== 200) {
    console.error(`FAIL: GET /test-bundle.js returned ${jsRes.status}, expected 200`);
    passed = false;
  } else {
    console.log("PASS: GET /test-bundle.js returned 200");
  }

  const mime = jsRes.headers["content-type"];
  if (!mime || !mime.includes("javascript")) {
    console.error(`FAIL: JS content-type is "${mime}", expected javascript`);
    passed = false;
  } else {
    console.log(`PASS: JS content-type is "${mime}"`);
  }

  const wasmRes = await fetch(`http://localhost:${port}/tur_wasm_bg.wasm`);
  if (wasmRes.status !== 200) {
    console.error(`FAIL: GET /tur_wasm_bg.wasm returned ${wasmRes.status}, expected 200`);
    passed = false;
  } else {
    console.log("PASS: GET /tur_wasm_bg.wasm returned 200");
  }

  const wasmMime = wasmRes.headers["content-type"];
  if (!wasmMime || !wasmMime.includes("wasm")) {
    console.error(`FAIL: wasm content-type is "${wasmMime}", expected wasm`);
    passed = false;
  } else {
    console.log(`PASS: wasm content-type is "${wasmMime}"`);
  }

  child.kill("SIGTERM");
  console.log("SIGTERM sent to CLI");

  if (passed) {
    console.log("\nAll tests passed");
    process.exit(0);
  } else {
    console.error("\nSome tests failed");
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
