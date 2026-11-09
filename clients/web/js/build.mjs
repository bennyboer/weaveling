import { build } from "esbuild";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "public", "js");

const result = await build({
  entryPoints: {
    shell: join(here, "shell.src.js"),
    prose: join(here, "prose.src.js"),
  },
  bundle: true,
  splitting: true,
  format: "esm",
  minify: true,
  target: "es2022",
  outdir: out,
  chunkNames: "[name]-[hash]",
  write: false,
});

mkdirSync(out, { recursive: true });
const built = new Set();
for (const file of result.outputFiles) {
  const name = basename(file.path);
  built.add(name);
  const unchanged = existsSync(file.path) && readFileSync(file.path).equals(Buffer.from(file.contents));
  if (!unchanged) {
    writeFileSync(file.path, file.contents);
  }
  console.log(`${name}  ${file.contents.length} bytes${unchanged ? " (unchanged)" : " (written)"}`);
}
for (const name of readdirSync(out)) {
  if (!built.has(name)) {
    rmSync(join(out, name));
  }
}
