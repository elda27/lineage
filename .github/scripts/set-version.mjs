// Synchronizes every packaged crate/app with the repository's VERSION file.
//
//   node .github/scripts/set-version.mjs
//   node .github/scripts/set-version.mjs --check
//   node .github/scripts/set-version.mjs --set 1.2.3
//
// The application/updater version remains SemVer. WiX gets a separate numeric
// ProductVersion (major.minor.patch.build), because MSI does not accept SemVer
// prerelease syntax such as `0.0.7-42` directly.

import { readFileSync, writeFileSync } from "node:fs";

const args = process.argv.slice(2);
const check = args[0] === "--check";
const setVersion = args[0] === "--set" ? args[1] : undefined;
if ((args.length && !check && args[0] !== "--set") || (check && args.length !== 1) || (args[0] === "--set" && args.length !== 2)) {
  console.error("usage: set-version.mjs [--check | --set <semver>]");
  process.exit(1);
}

const version = setVersion ?? readFileSync("VERSION", "utf8").trim();
const versionMatch = version.match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/);
if (!versionMatch) {
  console.error(`invalid version: ${version || "nothing"}`);
  process.exit(1);
}

const [, majorText, minorText, patchText, prerelease] = versionMatch;
const major = Number(majorText);
const minor = Number(minorText);
const patch = Number(patchText);
const build = prerelease && /^\d+$/.test(prerelease) ? Number(prerelease) : 0;

// Tauri/WiX limits: major/minor <= 255, patch/build <= 65535.
if (major > 255 || minor > 255 || patch > 65535 || build > 65535) {
  console.error(`version ${version} cannot be represented as an MSI ProductVersion`);
  process.exit(1);
}

const wixVersion = `${major}.${minor}.${patch}.${build}`;

if (setVersion) writeFileSync("VERSION", `${version}\n`);

const changes = [];

const writeOrCheck = (path, current, next) => {
  if (current === next) return;
  if (check) changes.push(path);
  else writeFileSync(path, next);
};

const setJsonVersion = (path, configure) => {
  const json = JSON.parse(readFileSync(path, "utf8"));
  const current = `${JSON.stringify(json, null, 2)}\n`;
  json.version = version;
  configure?.(json);
  writeOrCheck(path, current, `${JSON.stringify(json, null, 2)}\n`);
};

// Replaces the `version` key of the [package] table only, so dependency
// versions further down the manifest are left alone.
const setCargoVersion = (path) => {
  const lines = readFileSync(path, "utf8").split(/\r?\n/);
  let inPackage = false;
  let done = false;
  const patched = lines.map((line) => {
    if (/^\s*\[/.test(line)) inPackage = line.trim() === "[package]";
    if (inPackage && !done && /^\s*version\s*=\s*"/.test(line)) {
      done = true;
      return `version = "${version}"`;
    }
    return line;
  });
  if (!done) throw new Error(`no [package] version found in ${path}`);
  writeOrCheck(path, lines.join("\n"), patched.join("\n"));
};

setJsonVersion("fullos/package.json");
setJsonVersion("fullos/src-tauri/tauri.conf.json", (json) => {
  json.bundle ??= {};
  json.bundle.windows ??= {};
  json.bundle.windows.wix ??= {};
  json.bundle.windows.wix.version = wixVersion;
});
setCargoVersion("fullos/src-tauri/Cargo.toml");
setCargoVersion("minos/Cargo.toml");
setCargoVersion("agentos/Cargo.toml");
setCargoVersion("lineage-core/Cargo.toml");

if (changes.length) {
  console.error(`version ${version} is not synchronized: ${changes.join(", ")}`);
  process.exit(1);
}

console.log(check ? `version ${version} is synchronized (WiX ${wixVersion})` : `version synchronized to ${version} (WiX ${wixVersion})`);
