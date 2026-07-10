type ClipboardEnvironment = {
  clipboard?: Pick<Clipboard, "writeText">;
  document: Document;
  secure: boolean;
};

function defaultEnvironment(): ClipboardEnvironment {
  return {
    clipboard: navigator.clipboard,
    document,
    secure: window.isSecureContext,
  };
}

export async function copyText(text: string, environment = defaultEnvironment()) {
  if (environment.secure && environment.clipboard?.writeText) {
    try {
      await environment.clipboard.writeText(text);
      return;
    } catch {
      // Some embedded browsers expose Clipboard API but reject the operation.
    }
  }

  const textarea = environment.document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  environment.document.body.appendChild(textarea);
  textarea.select();
  const copied = environment.document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("复制失败");
}
