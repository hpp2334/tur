import { FSWatcher, watch } from "chokidar";

export class Watcher {
  private watcher: FSWatcher | null = null;

  watch(paths: string | string[], onChange: () => Promise<void>): void {
    this.watcher = watch(paths, {
      ignoreInitial: true,
      awaitWriteFinish: { stabilityThreshold: 100 },
    });
    this.watcher.on("all", (event: string, filepath: string) => {
      console.log(`[${new Date().toLocaleTimeString()}] ${filepath} (${event}) — reloading`);
      onChange().catch((e: unknown) => console.error("Watcher error:", e));
    });
  }

  close(): void {
    this.watcher?.close();
  }
}
