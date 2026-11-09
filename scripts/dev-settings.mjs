import { randomBytes } from "node:crypto";
import { existsSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const key = resolve(root, "weaveling.dev.key");
const settings = resolve(root, "weaveling.toml");

if (existsSync(key)) {
  console.log("kept weaveling.dev.key: a new one would lock out every second factor set up with it");
} else {
  writeFileSync(key, `${randomBytes(32).toString("hex")}\n`);
  console.log("wrote weaveling.dev.key");
}

if (existsSync(settings)) {
  console.log("kept weaveling.toml: see that it names totp_key_file = \"weaveling.dev.key\" and public_url = \"http://localhost:3000\"");
} else {
  writeFileSync(
    settings,
    `# Your own settings for running the server by hand from here; git ignores
# this file. weaveling.example.toml names every key.

[server]
# Open the client at this address, not at 127.0.0.1, or no passkey can be added.
public_url = "http://localhost:3000"

[accounts]
# enabled = true
totp_key_file = "weaveling.dev.key"
`,
  );
  console.log("wrote weaveling.toml");
}
