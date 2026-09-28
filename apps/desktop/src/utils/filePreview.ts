// Only passive raster formats are displayed inline. SVG/HTML remain downloads.
export function isPreviewImage(name: string) {
  return /\.(png|jpe?g|gif|webp|avif|bmp|ico)$/i.test(name);
}
