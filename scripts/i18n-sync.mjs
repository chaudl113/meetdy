#!/usr/bin/env node
/**
 * i18n-sync.mjs
 *
 * Syncs missing keys from the English locale into all other locales,
 * using the English value as a placeholder (prefixed with [TODO]).
 *
 * Usage:
 *   node scripts/i18n-sync.mjs [--check]
 *
 * Options:
 *   --check   Exit with code 1 if any locale is missing keys (CI mode, no writes).
 */

import { readFileSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const LOCALES_DIR = join(ROOT, "src", "i18n", "locales");
const CHECK_MODE = process.argv.includes("--check");

const LOCALES = ["de", "es", "fr", "it", "ja", "pl", "ru", "vi", "zh"];

/** Flatten a nested object into dot-separated key paths. */
function flatten(obj, prefix = "") {
  return Object.entries(obj).flatMap(([k, v]) => {
    const path = prefix ? `${prefix}.${k}` : k;
    return typeof v === "object" && v !== null && !Array.isArray(v)
      ? flatten(v, path)
      : [[path, v]];
  });
}

/** Set a value at a dot-separated path in a nested object, creating nodes as needed. */
function setNested(obj, path, value) {
  const parts = path.split(".");
  let cur = obj;
  for (let i = 0; i < parts.length - 1; i++) {
    if (typeof cur[parts[i]] !== "object" || cur[parts[i]] === null) {
      cur[parts[i]] = {};
    }
    cur = cur[parts[i]];
  }
  cur[parts[parts.length - 1]] = value;
}

const en = JSON.parse(
  readFileSync(join(LOCALES_DIR, "en", "translation.json"), "utf8")
);
const enFlat = new Map(flatten(en));

let anyMissing = false;
let totalMissing = 0;

for (const locale of LOCALES) {
  const filePath = join(LOCALES_DIR, locale, "translation.json");
  let locObj;
  try {
    locObj = JSON.parse(readFileSync(filePath, "utf8"));
  } catch {
    console.error(`[i18n-sync] Cannot read ${filePath}`);
    process.exit(1);
  }

  const locFlat = new Map(flatten(locObj));
  const missing = [...enFlat.keys()].filter((k) => !locFlat.has(k));

  if (missing.length === 0) {
    console.log(`✅ ${locale}: complete`);
    continue;
  }

  anyMissing = true;
  totalMissing += missing.length;
  console.log(`⚠️  ${locale}: ${missing.length} missing keys`);

  if (!CHECK_MODE) {
    // Fill missing keys with English value prefixed by [TODO].
    for (const key of missing) {
      const enVal = enFlat.get(key);
      const placeholder =
        typeof enVal === "string" ? `[TODO] ${enVal}` : enVal;
      setNested(locObj, key, placeholder);
    }
    writeFileSync(filePath, JSON.stringify(locObj, null, 2) + "\n", "utf8");
    console.log(`   → filled ${missing.length} keys with [TODO] placeholders`);
  } else {
    // In check mode, print a sample of missing keys.
    const sample = missing.slice(0, 5);
    for (const k of sample) {
      console.log(`   • ${k}`);
    }
    if (missing.length > 5) {
      console.log(`   … and ${missing.length - 5} more`);
    }
  }
}

if (CHECK_MODE) {
  if (anyMissing) {
    console.error(
      `\n❌ ${totalMissing} missing i18n keys across locales. Run: node scripts/i18n-sync.mjs`
    );
    process.exit(1);
  } else {
    console.log("\n✅ All locales are complete.");
    process.exit(0);
  }
} else if (anyMissing) {
  console.log(
    `\nDone. Filled ${totalMissing} missing keys. Translate [TODO] values before release.`
  );
}
