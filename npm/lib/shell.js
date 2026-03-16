const fs = require("fs");
const path = require("path");
const os = require("os");
const { BIN_DIR } = require("./platform");

const MARKER = "# yessir";
const LINE = `export PATH="${BIN_DIR}:$PATH"`;
const BLOCK = `\n${MARKER}\n${LINE}\n`;

// Shell profiles to check, in priority order per shell
const PROFILES = {
  zsh: [".zshrc"],
  bash: [".bashrc", ".bash_profile", ".profile"],
};

// Detect which shell the user is running
function detectShell() {
  const shell = process.env.SHELL || "";
  if (shell.includes("zsh")) return "zsh";
  if (shell.includes("bash")) return "bash";
  return "zsh"; // default on macOS
}

// Find the right profile file: first existing one, or first in list (to create)
function getProfilePath() {
  const shell = detectShell();
  const candidates = PROFILES[shell] || PROFILES.zsh;
  const home = os.homedir();

  for (const name of candidates) {
    const p = path.join(home, name);
    if (fs.existsSync(p)) return p;
  }

  // None exist, use the first candidate
  return path.join(home, candidates[0]);
}

function isPathConfigured() {
  const profilePath = getProfilePath();
  try {
    const content = fs.readFileSync(profilePath, "utf8");
    return content.includes(MARKER);
  } catch {
    return false;
  }
}

function addToPath() {
  if (isPathConfigured()) return null;

  const profilePath = getProfilePath();

  let content = "";
  try {
    content = fs.readFileSync(profilePath, "utf8");
  } catch (err) {
    if (err.code !== "ENOENT") throw err;
  }

  // Append block, ensure preceding newline
  const suffix = content.length > 0 && !content.endsWith("\n") ? "\n" : "";
  fs.writeFileSync(profilePath, content + suffix + BLOCK);

  return profilePath;
}

function removeFromPath() {
  const home = os.homedir();
  const allProfiles = [...new Set([...PROFILES.zsh, ...PROFILES.bash])];
  let removed = false;

  for (const name of allProfiles) {
    const p = path.join(home, name);
    try {
      const content = fs.readFileSync(p, "utf8");
      if (!content.includes(MARKER)) continue;

      // Remove the block (marker line + export line + surrounding blank lines)
      const cleaned = content
        .replace(new RegExp(`\\n?${MARKER.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\n[^\\n]*\\n?`, "g"), "\n")
        .replace(/\n{3,}/g, "\n\n"); // collapse excessive blank lines

      fs.writeFileSync(p, cleaned);
      console.log(`Removed PATH from ${p}`);
      removed = true;
    } catch {
      // file doesn't exist, skip
    }
  }

  return removed;
}

module.exports = {
  addToPath,
  removeFromPath,
  isPathConfigured,
  getProfilePath,
};
