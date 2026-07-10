export function detectClientSource(userAgent = navigator.userAgent) {
  if (/MicroMessenger/i.test(userAgent)) return "微信内置浏览器";
  if (/EdgA|EdgiOS|Edg\//i.test(userAgent)) return "Edge";
  if (/CriOS|Chrome/i.test(userAgent)) return "Chrome";
  if (/FxiOS|Firefox/i.test(userAgent)) return "Firefox";
  if (/Safari/i.test(userAgent) && /Version/i.test(userAgent)) return "Safari";
  return "其他浏览器";
}
