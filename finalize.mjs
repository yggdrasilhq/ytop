#!/usr/bin/env node
/**
 * ynpm finalize - the yggdrasilhq-package postinstall.
 *
 * Copies this package's platform binary over the entry shim, marks it
 * executable, and verifies it RUNS (--version). Exits non-zero otherwise:
 * the ynpm install gate refuses a package whose binary cannot run, exactly
 * like the managed-CLI provisioner's publish gate.
 *
 * First-party script: ships in yggdrasilhq's own package, from the same repo
 * as the binary. The boundary is the vendor-script boundary: HOME intact, no
 * privilege escalation, stdin closed by the installer.
 *
 * ESM: the file is .mjs (npm runs postinstall with the package's type), so
 * `require` is undefined here — import instead. Live-fenced: the first
 * `npm i -g @ygghq/ytop` died exactly there.
 */
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const NAME = process.env.YNPM_BIN_NAME;
const PACKAGE = process.env.YNPM_PACKAGE_NAME;
const PLATFORM = process.env.YNPM_PLATFORM;

function fail(message) {
  console.error(`ynpm finalize: ${message}`);
  process.exit(1);
}

if (!NAME || !PACKAGE || !PLATFORM) {
  fail("YNPM_BIN_NAME / YNPM_PACKAGE_NAME / YNPM_PLATFORM must be set by the installer");
}

const shimPath = path.join(__dirname, "bin", NAME);
const platformBinary = path.join(
  __dirname, "..", "..", "..", `${PACKAGE}-${PLATFORM}`, "bin", NAME
);

if (!fs.existsSync(platformBinary)) {
  fail(`${PACKAGE}-${PLATFORM} is not installed beside this package - the platform binary is required on this machine`);
}

fs.mkdirSync(path.dirname(shimPath), { recursive: true });
fs.copyFileSync(platformBinary, shimPath);
fs.chmodSync(shimPath, 0o755);

try {
  execFileSync(shimPath, ["--version"], { stdio: "ignore", timeout: 30000 });
} catch (error) {
  fail(`${NAME} does not run after finalize (${error.status ?? error.message})`);
}
