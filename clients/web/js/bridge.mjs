import { readFileSync } from "node:fs";
import { join } from "node:path";

const OFFERED_IN = {
  SyncConnection: "sync.js",
  ProseEditor: join("editor", "editor.js"),
};

function asked(bridge) {
  const asked = [];
  const declarations = bridge.matchAll(
    /#\[wasm_bindgen\(([^)]*)\)\]\s*pub fn (\w+)\(\s*this: &(\w+)/g,
  );
  for (const [, attributes, name, type] of declarations) {
    if (!/\bmethod\b/.test(attributes)) {
      continue;
    }
    const renamed = attributes.match(/js_name = (\w+)/);
    asked.push({ type, method: renamed ? renamed[1] : name });
  }

  return asked;
}

function offered(source, type) {
  const start = source.indexOf(`export class ${type} `);
  if (start < 0) {
    return new Set();
  }
  const end = source.indexOf("\n}\n", start);
  const body = source.slice(start, end < 0 ? undefined : end);

  return new Set(
    [...body.matchAll(/^ {2}(\w+)\([^)]*\) \{$/gm)].map(([, name]) => name),
  );
}

export function missingFromTheBridge(here, bridgePath) {
  const bridge = readFileSync(bridgePath, "utf8");
  const sources = new Map();
  const missing = [];
  for (const { type, method } of asked(bridge)) {
    const file = OFFERED_IN[type];
    if (!file) {
      missing.push(`${type}.${method} (no script is known to offer ${type})`);
      continue;
    }
    if (!sources.has(type)) {
      sources.set(type, offered(readFileSync(join(here, file), "utf8"), type));
    }
    if (!sources.get(type).has(method)) {
      missing.push(`${type}.${method} (asked in prose.rs, missing in ${file})`);
    }
  }

  return missing;
}
