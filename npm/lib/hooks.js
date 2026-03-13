const fs = require("fs");
const path = require("path");
const os = require("os");
const { BIN_DIR, shellQuote, atomicWriteFileSync } = require("./platform");

const SETTINGS_PATH = path.join(os.homedir(), ".claude", "settings.json");

const HOOK_EVENTS = [
  "SessionStart",
  "UserPromptSubmit",
  "PreToolUse",
  "PermissionRequest",
  "PostToolUse",
  "PostToolUseFailure",
  "Stop",
  "SessionEnd",
];

function hookCommand(event) {
  const binPath = path.join(BIN_DIR, "yessir-hook");
  // Quote the path to handle spaces in $HOME
  return `${shellQuote(binPath)} ${event}`;
}

// Exact match: must be type "command" with our exact command string
function isOurHook(entry, event) {
  return (
    entry &&
    entry.type === "command" &&
    entry.command === hookCommand(event)
  );
}

// Read settings.json with fail-closed semantics:
// - File doesn't exist → return {} (normal, first create)
// - File exists but parse/read fails → throw (refuse to overwrite)
function readSettings() {
  try {
    const content = fs.readFileSync(SETTINGS_PATH, "utf8");
    return JSON.parse(content);
  } catch (err) {
    if (err.code === "ENOENT") {
      return {};
    }
    throw new Error(
      `Cannot read ${SETTINGS_PATH}: ${err.message}\nPlease fix or remove this file manually.`
    );
  }
}

// Atomic write: temp file + rename
function writeSettings(settings) {
  const dir = path.dirname(SETTINGS_PATH);
  fs.mkdirSync(dir, { recursive: true });
  atomicWriteFileSync(SETTINGS_PATH, JSON.stringify(settings, null, 2) + "\n");
}

function installHooks() {
  const settings = readSettings();
  if (!settings.hooks) {
    settings.hooks = {};
  }

  for (const event of HOOK_EVENTS) {
    if (!Array.isArray(settings.hooks[event])) {
      settings.hooks[event] = [];
    }

    // Remove any existing yessir hooks (exact match, handles path change / reinstall)
    settings.hooks[event] = settings.hooks[event].filter(
      (entry) => !isOurHook(entry, event)
    );

    // Add our hook
    settings.hooks[event].push({
      type: "command",
      command: hookCommand(event),
    });
  }

  writeSettings(settings);
  console.log(`Hooks configured in ${SETTINGS_PATH}`);
}

function uninstallHooks() {
  if (!fs.existsSync(SETTINGS_PATH)) {
    console.log("No settings.json found, nothing to clean up.");
    return;
  }

  const settings = readSettings();
  if (!settings.hooks) {
    console.log("No hooks configured, nothing to clean up.");
    return;
  }

  let removed = 0;

  for (const event of HOOK_EVENTS) {
    if (!Array.isArray(settings.hooks[event])) continue;

    const before = settings.hooks[event].length;
    settings.hooks[event] = settings.hooks[event].filter(
      (entry) => !isOurHook(entry, event)
    );
    removed += before - settings.hooks[event].length;

    // Clean up empty arrays
    if (settings.hooks[event].length === 0) {
      delete settings.hooks[event];
    }
  }

  // Clean up empty hooks object
  if (Object.keys(settings.hooks).length === 0) {
    delete settings.hooks;
  }

  writeSettings(settings);
  console.log(`Removed ${removed} hook(s) from ${SETTINGS_PATH}`);
}

function isHooksInstalled() {
  try {
    const settings = readSettings();
    if (!settings.hooks) return false;

    return HOOK_EVENTS.every((event) => {
      const hooks = settings.hooks[event];
      return (
        Array.isArray(hooks) &&
        hooks.some((e) => isOurHook(e, event))
      );
    });
  } catch {
    return false;
  }
}

module.exports = {
  installHooks,
  uninstallHooks,
  isHooksInstalled,
};
