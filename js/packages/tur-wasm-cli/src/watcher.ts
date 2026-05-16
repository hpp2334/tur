import chokidar from "chokidar";

export class Watcher {
  private watcher: chokidar.FSWatcher | null = null;

  watch(paths: string | string[], onChange: () => Promise<void>): void {
    this.watcher = chokidar.watch(paths, {
      ignoreInitial: true,
      awaitWriteFinish: { stabilityThreshold: 100 },
    });
    this.watcher.on("all", (event, filepath) => {
      console.log(`[${new Date().toLocaleTimeString()}] ${filepath} (${event}) — reloading`);
      onChange().catch((e) => console.error(`Watcher error: ${e}`));
    });
  }

  close(): void {
    this.watcher?.close();
  }
}
