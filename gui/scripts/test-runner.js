import { spawn } from "node:child_process";

/**
 * A tiny wrapper to keep `npm test -- --runInBand` working.
 * Vitest does not understand Jest's `--runInBand`, so we strip it and translate
 * to Vitest's equivalent of single-threaded execution.
 */
const rawArgs = process.argv.slice(2);

let requestedInBand = false;
const cleanedArgs = [];

for (const arg of rawArgs) {
  if (arg === "--runInBand") {
    requestedInBand = true;
    continue;
  }
  cleanedArgs.push(arg);
}

// Run in a single thread when Jest's `--runInBand` is supplied.
if (requestedInBand && !cleanedArgs.some((arg) => arg.startsWith("--threads"))) {
  cleanedArgs.push("--threads=false");
}

const child = spawn("npm", ["exec", "vitest", ...cleanedArgs], {
  stdio: "inherit",
  env: process.env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});

child.on("error", (error) => {
  console.error(error);
  process.exit(1);
});
