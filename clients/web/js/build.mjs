import { build } from "esbuild";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const bundle = join(here, "editor.bundle.js");

const result = await build({
  entryPoints: [join(here, "editor.src.js")],
  bundle: true,
  format: "esm",
  minify: true,
  target: "es2022",
  write: false,
});

const [built] = result.outputFiles;
const unchanged = existsSync(bundle) && readFileSync(bundle).equals(Buffer.from(built.contents));

if (unchanged) {
  console.log(`editor.bundle.js  ${built.contents.length} bytes (unchanged)`);
} else {
  writeFileSync(bundle, built.contents);
  console.log(`editor.bundle.js  ${built.contents.length} bytes (written)`);
}
