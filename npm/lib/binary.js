const fs = require("fs");
const path = require("path");
const https = require("https");
const crypto = require("crypto");
const { execSync } = require("child_process");
const {
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
  semverCompare,
  atomicWriteFileSync,
} = require("./platform");

// The npm package version — authoritative for initial install
const PACKAGE_VERSION = require("../package.json").version;

function isInstalled() {
  return (
    fs.existsSync(path.join(BIN_DIR, "yessir")) &&
    fs.existsSync(path.join(BIN_DIR, "yessir-hook"))
  );
}

function readVersionInfo() {
  try {
    return JSON.parse(fs.readFileSync(VERSION_FILE, "utf8"));
  } catch {
    return null;
  }
}

function writeVersionInfo(version) {
  fs.mkdirSync(INSTALL_DIR, { recursive: true });
  atomicWriteFileSync(
    VERSION_FILE,
    JSON.stringify(
      { version, lastCheck: Math.floor(Date.now() / 1000) },
      null,
      2
    )
  );
}

// Fetch the latest release version from GitHub API (with timeout)
function fetchLatestVersion() {
  return new Promise((resolve) => {
    const options = {
      hostname: "api.github.com",
      path: `/repos/${GITHUB_REPO}/releases/latest`,
      headers: { "User-Agent": "yessir-cli" },
      timeout: HTTP_TIMEOUT_MS,
    };

    const handleResponse = (res) => {
      let data = "";
      res.on("data", (chunk) => (data += chunk));
      res.on("end", () => {
        try {
          const tag = JSON.parse(data).tag_name;
          resolve(tag ? tag.replace(/^v/, "") : null);
        } catch {
          resolve(null);
        }
      });
    };

    const req = https.get(options, (res) => {
      // Handle redirect
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        const redirectReq = https.get(
          res.headers.location,
          { headers: options.headers, timeout: HTTP_TIMEOUT_MS },
          handleResponse
        );
        redirectReq.on("error", () => resolve(null));
        redirectReq.on("timeout", () => { redirectReq.destroy(); resolve(null); });
        return;
      }
      handleResponse(res);
    });
    req.on("error", () => resolve(null));
    req.on("timeout", () => { req.destroy(); resolve(null); });
  });
}

// Download checksum file for a release
function fetchChecksums(version) {
  const url = getChecksumUrl(version);
  try {
    const result = execSync(
      `curl -fSL --max-time ${CURL_TIMEOUT_SECS} "${url}"`,
      { encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }
    );
    // Parse: each line is "sha256  filename"
    const checksums = {};
    for (const line of result.trim().split("\n")) {
      const [hash, file] = line.trim().split(/\s+/);
      if (hash && file) checksums[file] = hash;
    }
    return checksums;
  } catch {
    return null; // checksum file not available (older releases)
  }
}

// Download and extract the binary tarball (with temp dir + checksum + atomic swap)
function downloadBinary(version) {
  const platformKey = getPlatformKey();
  if (!platformKey) {
    throw new Error(
      `Unsupported platform: ${process.platform}-${process.arch}. Only macOS (arm64/x64) is supported.`
    );
  }

  const url = getDownloadUrl(version, platformKey);
  const tarballName = `yessir-${platformKey}.tar.gz`;
  console.log(`Downloading yessir v${version} for ${platformKey}...`);

  // Prepare temp directory
  fs.mkdirSync(TMP_DIR, { recursive: true });
  const tarball = path.join(TMP_DIR, tarballName);
  const extractDir = path.join(TMP_DIR, "extract");

  try {
    // Download with timeout
    execSync(
      `curl -fSL --progress-bar --connect-timeout 10 --max-time ${CURL_TIMEOUT_SECS} -o "${tarball}" "${url}"`,
      { stdio: "inherit" }
    );

    // Verify checksum if available
    const checksums = fetchChecksums(version);
    if (checksums && checksums[tarballName]) {
      const expected = checksums[tarballName];
      const fileBuffer = fs.readFileSync(tarball);
      const actual = crypto.createHash("sha256").update(fileBuffer).digest("hex");
      if (actual !== expected) {
        throw new Error(
          `Checksum mismatch for ${tarballName}:\n  expected: ${expected}\n  got:      ${actual}`
        );
      }
    }

    // Extract to temp dir
    fs.mkdirSync(extractDir, { recursive: true });
    execSync(`tar -xzf "${tarball}" -C "${extractDir}"`, { stdio: "inherit" });

    // Verify extracted binaries exist
    const newYessir = path.join(extractDir, "yessir");
    const newHook = path.join(extractDir, "yessir-hook");
    if (!fs.existsSync(newYessir) || !fs.existsSync(newHook)) {
      throw new Error("Tarball did not contain expected binaries (yessir, yessir-hook).");
    }

    // chmod +x
    fs.chmodSync(newYessir, 0o755);
    fs.chmodSync(newHook, 0o755);

    // Atomic swap: rename-aside old, rename-in new
    fs.mkdirSync(BIN_DIR, { recursive: true });
    const oldYessir = path.join(BIN_DIR, "yessir");
    const oldHook = path.join(BIN_DIR, "yessir-hook");
    const backupYessir = oldYessir + ".old";
    const backupHook = oldHook + ".old";

    // Rename old binaries aside (if they exist)
    try { fs.renameSync(oldYessir, backupYessir); } catch {}
    try { fs.renameSync(oldHook, backupHook); } catch {}

    // Move new binaries in
    fs.renameSync(newYessir, oldYessir);
    fs.renameSync(newHook, oldHook);

    // Clean up backups
    try { fs.unlinkSync(backupYessir); } catch {}
    try { fs.unlinkSync(backupHook); } catch {}

    writeVersionInfo(version);
    console.log(`Installed yessir v${version} to ${BIN_DIR}`);
  } finally {
    // Clean up temp dir
    try { fs.rmSync(TMP_DIR, { recursive: true, force: true }); } catch {}
  }
}

// Ensure binary is installed and up-to-date
// Options:
//   force: true — always re-download (for `update` command)
async function ensureBinary({ force = false } = {}) {
  const versionInfo = readVersionInfo();
  const now = Math.floor(Date.now() / 1000);

  // Not installed at all — must install package version
  if (!isInstalled() || !versionInfo) {
    downloadBinary(PACKAGE_VERSION);
    return;
  }

  // Force mode (update command) — always fetch and download latest
  if (force) {
    const latest = await fetchLatestVersion();
    if (latest) {
      downloadBinary(latest);
    } else {
      // Can't reach GitHub, reinstall package version
      downloadBinary(PACKAGE_VERSION);
    }
    return;
  }

  // Installed binary is older than the npm package version — upgrade immediately
  if (semverCompare(PACKAGE_VERSION, versionInfo.version) > 0) {
    console.log(`Upgrading yessir v${versionInfo.version} → v${PACKAGE_VERSION}...`);
    downloadBinary(PACKAGE_VERSION);
    return;
  }

  // Within cooldown — skip update check
  const elapsed = now - (versionInfo.lastCheck || 0);
  if (elapsed < UPDATE_CHECK_INTERVAL) {
    return;
  }

  // Check for newer release
  const latest = await fetchLatestVersion();
  if (latest && semverCompare(latest, versionInfo.version) > 0) {
    console.log(`Update available: v${versionInfo.version} → v${latest}`);
    downloadBinary(latest);
  } else {
    // Refresh lastCheck timestamp
    writeVersionInfo(versionInfo.version);
  }
}

function uninstallBinary() {
  if (fs.existsSync(INSTALL_DIR)) {
    fs.rmSync(INSTALL_DIR, { recursive: true, force: true });
    console.log(`Removed ${INSTALL_DIR}`);
  } else {
    console.log("yessir is not installed.");
  }
}

module.exports = {
  isInstalled,
  ensureBinary,
  downloadBinary,
  uninstallBinary,
  readVersionInfo,
  BIN_DIR,
};
