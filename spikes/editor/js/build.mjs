import { build } from "esbuild";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

const result = await build({
  entryPoints: [join(here, "editor.src.js")],
  outfile: join(here, "editor.bundle.js"),
  bundle: true,
  format: "esm",
  minify: true,
  target: "es2022",
  metafile: true,
});

const [outputs] = Object.values(result.metafile.outputs);
console.log(`editor.bundle.js  ${outputs.bytes} bytes`);
