// Rewrites the version of every packaged crate/app to the one given on argv.
//
//   node .github/scripts/set-version.mjs 1.2.3
//
// The release workflow derives the version from the git tag and runs this
// before building so that the MSI version, the Tauri bundle version and the
// `version` field in the updater's latest.json all agree with the release.
// It is CI-only: the rewritten files are never committed back.

import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(?:[-+].*)?$/.test(version)) {
  console.error(`usage: set-version.mjs <semver>  (got: ${version ?? "nothing"})`);
  process.exit(1);
}

const setJsonVersion = (path) => {
  const json = JSON.parse(readFileSync(path, "utf8"));
  json.version = version;
  writeFileSync(path, `${JSON.stringify(json, null, 2)}\n`);
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
  writeFileSync(path, patched.join("\n"));
};

setJsonVersion("fullos/package.json");
setJsonVersion("fullos/src-tauri/tauri.conf.json");
setCargoVersion("fullos/src-tauri/Cargo.toml");
setCargoVersion("minos/Cargo.toml");

console.log(`version set to ${version}`);
