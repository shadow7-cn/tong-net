import { expect, it } from "vitest";
import { isPreviewImage } from "./filePreview";

it("previews common raster images but never active document formats", () => {
  for (const name of ["截图.PNG", "photo.jpg", "a.jpeg", "a.webp", "a.gif", "a.avif", "a.bmp", "a.ico"]) {
    expect(isPreviewImage(name)).toBe(true);
  }
  for (const name of ["a.svg", "a.html", "a.png.html", "a.pdf", "a.exe", "png"]) {
    expect(isPreviewImage(name)).toBe(false);
  }
});
