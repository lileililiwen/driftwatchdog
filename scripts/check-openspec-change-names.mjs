#!/usr/bin/env node
import { readdir } from "node:fs/promises";
import { join } from "node:path";

const pattern = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const root = process.argv[2] ?? join(process.cwd(), "openspec", "changes");
let entries;
try { entries = await readdir(root, { withFileTypes: true }); }
catch (error) {
  if (error.code === "ENOENT") process.exit(0);
  console.error(`Cannot inspect ${root}: ${error.message}`); process.exit(1);
}
const invalid = entries.filter((entry) => entry.isDirectory() && entry.name !== "archive" && !entry.name.startsWith(".") && !pattern.test(entry.name)).map((entry) => entry.name);
if (invalid.length) {
  for (const name of invalid) console.error(`Invalid active OpenSpec change name: ${name}`);
  process.exit(1);
}
console.log(`OpenSpec change names valid: ${root}`);
