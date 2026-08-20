// Selects the updater feed embedded into the Tauri application at build time.
//
//   node .github/scripts/set-updater-channel.mjs stable
//   node .github/scripts/set-updater-channel.mjs nightly
//
// Stable builds follow GitHub's latest non-prerelease release. Nightly builds
// must use the fixed `nightly` prerelease because /releases/latest excludes
// prereleases by design.

import { readFileSync, writeFileSync } from "node:fs";

const channel = process.argv[2];
if (!channel || process.argv.length !== 3 || !["stable", "nightly"].includes(channel)) {
  console.error("usage: set-updater-channel.mjs <stable|nightly>");
  process.exit(1);
}

const endpoints = {
  stable: "https://github.com/elda27/lineage/releases/latest/download/latest.json",
  nightly: "https://github.com/elda27/lineage/releases/download/nightly/latest.json",
};

const configPath = "fullos/src-tauri/tauri.conf.json";
const config = JSON.parse(readFileSync(configPath, "utf8"));
config.plugins ??= {};
config.plugins.updater ??= {};
config.plugins.updater.endpoints = [endpoints[channel]];
writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);

console.log(`updater channel set to ${channel}: ${endpoints[channel]}`);
