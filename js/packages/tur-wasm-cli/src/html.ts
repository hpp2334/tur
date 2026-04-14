import htmlTemplate from "./template.html?raw";

export function generateHtml(jsFilename: string): string {
  return htmlTemplate.replace("__JS_FILE__", jsFilename);
}
