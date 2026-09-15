export function selectEasyTierTarget(platform, arch, override = "") {
  const key = override || `${platform}-${arch}`;
  const targets = {
    "darwin-arm64": { os: "macos", arch: "aarch64", triple: "aarch64-apple-darwin" },
    "darwin-x64": { os: "macos", arch: "x86_64", triple: "x86_64-apple-darwin" },
    "win32-x64": { os: "windows", arch: "x86_64", triple: "x86_64-pc-windows-msvc" },
  };
  const target = targets[key];
  if (!target || !key.startsWith(`${platform}-`)) throw new Error(`不支持的 EasyTier 构建目标：${key}`);
  return {
    ...target,
    executables: platform === "win32" ? ["easytier-core.exe", "easytier-cli.exe"] : ["easytier-core", "easytier-cli"],
  };
}
