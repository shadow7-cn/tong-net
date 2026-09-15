import { createHash } from "node:crypto";
import { chmod, copyFile, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { spawnSync } from "node:child_process";
import { selectEasyTierTarget } from "./easytier-target.mjs";

const version = "2.6.4";
const target = selectEasyTierTarget(process.platform, process.arch, process.env.EASYTIER_TARGET);
target.asset = `easytier-${target.os}-${target.arch}-v${version}.zip`;

async function fetchJson(url) {
  const response = await fetch(url, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "tong-net-build",
      ...(process.env.GITHUB_TOKEN ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` } : {}),
    },
  });
  if (!response.ok) throw new Error(`GitHub API 请求失败：${response.status}`);
  return response.json();
}

async function findFile(directory, fileName) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      const nested = await findFile(path, fileName);
      if (nested) return nested;
    } else if (entry.name === fileName) {
      return path;
    }
  }
  return null;
}

const release = await fetchJson(
  `https://api.github.com/repos/EasyTier/EasyTier/releases/tags/v${version}`,
);
const asset = release.assets.find((item) => item.name === target.asset);
if (!asset) throw new Error(`EasyTier v${version} 中没有找到 ${target.asset}`);
if (!asset.digest?.startsWith("sha256:")) {
  throw new Error("GitHub Release 未提供 SHA-256，拒绝使用未校验的 Core");
}

const archiveResponse = await fetch(asset.browser_download_url, {
  headers: { "User-Agent": "tong-net-build" },
});
if (!archiveResponse.ok) throw new Error(`下载 EasyTier Core 失败：${archiveResponse.status}`);
const archive = Buffer.from(await archiveResponse.arrayBuffer());
const actualDigest = createHash("sha256").update(archive).digest("hex");
const expectedDigest = asset.digest.slice("sha256:".length);
if (actualDigest !== expectedDigest) throw new Error("EasyTier Core SHA-256 校验失败");

const temporary = await mkdtemp(join(tmpdir(), "tong-net-easytier-"));
const archivePath = join(temporary, basename(target.asset));
const extracted = join(temporary, "extracted");
await mkdir(extracted);
await writeFile(archivePath, archive);

const unzip = process.platform === "win32"
  ? spawnSync(
      "powershell",
      ["-NoProfile", "-Command", `Expand-Archive -LiteralPath '${archivePath}' -DestinationPath '${extracted}' -Force`],
      { stdio: "inherit" },
    )
  : spawnSync("unzip", ["-q", archivePath, "-d", extracted], { stdio: "inherit" });
if (unzip.status !== 0) throw new Error("解压 EasyTier Core 失败");

const binaryDirectory = join("apps", "desktop", "src-tauri", "binaries");
const extension = process.platform === "win32" ? ".exe" : "";
await mkdir(binaryDirectory, { recursive: true });
if (process.platform === "win32") {
  const runtimeDirectory = join(binaryDirectory, "windows");
  await mkdir(runtimeDirectory, { recursive: true });
  for (const name of ["wintun.dll", "Packet.dll", "WinDivert64.sys"]) {
    const source = await findFile(extracted, name);
    if (!source) throw new Error(`压缩包中没有找到 Windows 运行库 ${name}`);
    await copyFile(source, join(runtimeDirectory, name));
  }
}
for (const executable of target.executables) {
  const source = await findFile(extracted, executable);
  if (!source) throw new Error(`压缩包中没有找到 ${executable}`);
  const baseName = executable.replace(/\.exe$/, "");
  const destination = join(binaryDirectory, `${baseName}-${target.triple}${extension}`);
  await copyFile(source, destination);
  if (process.platform !== "win32") await chmod(destination, 0o755);
  console.log(`已准备 EasyTier v${version}: ${destination}`);
}
await rm(temporary, { recursive: true, force: true });
