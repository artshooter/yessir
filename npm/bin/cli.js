#!/usr/bin/env node

const path = require("path");
const { spawn } = require("child_process");
const { ensureBinary, uninstallBinary, readVersionInfo, isInstalled, BIN_DIR } = require("../lib/binary");
const { installHooks, uninstallHooks, isHooksInstalled } = require("../lib/hooks");
const { addToPath, removeFromPath, isPathConfigured } = require("../lib/shell");

const USAGE = `Usage: yessir <command>

Commands:
  start       Start the TUI dashboard (default)
  install     Download binary and configure hooks
  uninstall   Remove hooks and binary
  update      Force update to latest version
  status      Show installation status
`;

const command = process.argv[2] || "start";

async function main() {
  switch (command) {
    case "install": {
      await ensureBinary();
      installHooks();
      const profile = addToPath();
      console.log("\nyessir installed successfully!");
      if (profile) {
        console.log(`Added ~/.yessir/bin to PATH in ${profile}`);
        console.log("Restart your terminal, then run `yessir` to start the dashboard.");
      } else {
        console.log("Run `yessir` to start the dashboard.");
      }
      break;
    }

    case "uninstall":
      uninstallHooks();
      removeFromPath();
      uninstallBinary();
      console.log("\nyessir uninstalled.");
      break;

    case "update":
      await ensureBinary({ force: true });
      console.log("Update complete.");
      break;

    case "status": {
      const info = readVersionInfo();
      const installed = isInstalled();
      const hooksOk = isHooksInstalled();
      const pathOk = isPathConfigured();
      console.log(`Binary: ${installed ? `v${info?.version || "?"}  (${BIN_DIR})` : "not installed"}`);
      console.log(`Hooks:  ${hooksOk ? "configured" : "not configured"}`);
      console.log(`PATH:   ${pathOk ? "configured" : "not configured"}`);
      break;
    }

    case "start":
      await startTUI();
      break;

    default:
      console.error(`Unknown command: ${command}\n`);
      console.error(USAGE);
      process.exit(1);
  }
}

async function startTUI() {
  // Ensure binary + hooks are set up
  await ensureBinary();
  if (!isHooksInstalled()) {
    installHooks();
  }

  // Launch the TUI (use spawn, not spawnSync, to not block event loop)
  const binary = path.join(BIN_DIR, "yessir");
  const child = spawn(binary, [], {
    stdio: "inherit",
    env: { ...process.env },
  });

  child.on("error", (err) => {
    console.error(`Failed to launch ${binary}: ${err.message}`);
    process.exit(1);
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.exit(1);
    }
    process.exit(code ?? 1);
  });
}

main().catch((err) => {
  console.error(`Error: ${err.message}`);
  process.exit(1);
});
