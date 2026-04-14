import fsSync from "fs";
import path from "path";

export function watchFile(filePath: string, onChange: () => void): fsSync.FSWatcher {
  let lastTrigger = 0;
  const watcher = fsSync.watch(filePath, (eventType: string) => {
    if (eventType !== "change") return;
    const now = Date.now();
    if (now - lastTrigger < 300) return;
    lastTrigger = now;
    console.log(`[${new Date().toLocaleTimeString()}] ${path.basename(filePath)} changed — reloading`);
    onChange();
  });
  console.log(`Watching ${filePath}`);
  return watcher;
}
