const os = require("os");
const path = require("path");

const GITHUB_REPO = "artshooter/yessir";
const INSTALL_DIR = path.join(os.homedir(), ".yessir");
const BIN_DIR = path.join(INSTALL_DIR, "bin");
const VERSION_FILE = path.join(INSTALL_DIR, "version.json");
const TMP_DIR = path.join(INSTALL_DIR, "tmp");

// 1 hour cooldown between update checks
const UPDATE_CHECK_INTERVAL = 3600;

// Timeout for HTTP requests (version check) and curl downloads
const HTTP_TIMEOUT_MS = 5000;
const CURL_TIMEOUT_SECS = 60;

function getPlatformKey() {
  const arch = os.arch();
  const platform = os.platform();

  if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "darwin-x64";

  return null;
}

function getDownloadUrl(version, platformKey) {
  return `https://github.com/${GITHUB_REPO}/releases/download/v${version}/yessir-${platformKey}.tar.gz`;
}

function getChecksumUrl(version) {
  return `https://github.com/${GITHUB_REPO}/releases/download/v${version}/checksums.txt`;
}

// Shell-safe quoting for paths (handles spaces, special chars)
function shellQuote(s) {
  return "'" + s.replace(/'/g, "'\\''") + "'";
}

// Simple semver compare for strict x.y.z versions
// Returns 1 if a > b, -1 if a < b, 0 if equal
function semverCompare(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i++) {
    const na = pa[i] || 0;
    const nb = pb[i] || 0;
    if (isNaN(na) || isNaN(nb)) return 0; // reject non-numeric
    if (na > nb) return 1;
    if (na < nb) return -1;
  }
  return 0;
}

// Atomic write: write to temp file, then rename into place
function atomicWriteFileSync(filePath, data) {
  const dir = path.dirname(filePath);
  fs.mkdirSync(dir, { recursive: true });
  const tmp = filePath + ".tmp." + process.pid;
  fs.writeFileSync(tmp, data);
  fs.renameSync(tmp, filePath);
}

// Lazy require fs (avoid circular deps at module level)
const fs = require("fs");

module.exports = {
  GITHUB_REPO,
  INSTALL_DIR,
  BIN_DIR,
  VERSION_FILE,
  TMP_DIR,
  UPDATE_CHECK_INTERVAL,
  HTTP_TIMEOUT_MS,
  CURL_TIMEOUT_SECS,
  getPlatformKey,
  getDownloadUrl,
  getChecksumUrl,
  shellQuote,
  semverCompare,
  atomicWriteFileSync,
};
