export function getDefaultRoute(runningInTauri: boolean) {
  return runningInTauri ? "/desktop" : "/web";
}
