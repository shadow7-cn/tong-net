import type { EasyTierConfig, EasyTierMember } from "@/api/easytier";

export type EasyTierMemberRole = "local" | "shared" | "service" | "device";

export function getEasyTierMemberRole(member: EasyTierMember): EasyTierMemberRole {
  if (member.local) return "local";
  if (member.hostname.startsWith("PublicServer_")) return "shared";
  if (member.hostname.startsWith("同网互通服务-")) return "service";
  return "device";
}

export function continueInsecureConnection(
  values: EasyTierConfig,
  rememberConfirmation: (confirmed: EasyTierConfig) => void,
  connect: (confirmed: EasyTierConfig) => Promise<void>,
): void {
  const confirmed = { ...values, allowInsecureHttp: true };
  rememberConfirmation(confirmed);
  void connect(confirmed);
}
