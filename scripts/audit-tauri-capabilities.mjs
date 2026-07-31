import { readFile } from "node:fs/promises";

const path = new URL("../apps/desktop/src-tauri/capabilities/main.json", import.meta.url);
const capability = JSON.parse(await readFile(path, "utf8"));
const permissions = capability.permissions;

if (!Array.isArray(permissions)) throw new Error("Tauri capability permissions must be an array");

const forbidden = permissions.filter(
  (permission) =>
    typeof permission !== "string" ||
    permission === "core:default" ||
    permission.includes("shell") ||
    permission.includes("fs") ||
    permission.includes("image:allow-from-path") ||
    permission.includes("webview:allow-open-devtools"),
);

if (forbidden.length > 0) {
  throw new Error(`Forbidden renderer capabilities: ${forbidden.join(", ")}`);
}

// M0 needs no Tauri core/plugin permissions. Application commands are registered
// explicitly in the Rust invoke handler and do not require a broad core set.
if (permissions.length !== 0) {
  throw new Error(
    `Review every renderer permission before expanding M0; found ${permissions.length}`,
  );
}

console.log("Tauri renderer capability audit passed: no core/plugin permissions granted");
