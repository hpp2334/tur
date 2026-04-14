import http from "http";
import Koa from "koa";
import koaServe from "koa-static";
import { WebSocketServer } from "ws";

export class Server {
  private server: http.Server;
  private wss: WebSocketServer;
  private port: number;

  constructor(dir: string, port: number) {
    this.port = port;
    const app = new Koa();
    app.use(koaServe(dir));
    this.server = http.createServer(app.callback());
    this.wss = new WebSocketServer({ server: this.server, path: "/__ws" });
  }

  start(): void {
    this.server.listen(this.port, () => {
      console.log(`Serving at http://localhost:${this.port}`);
    });
  }

  broadcast(msg: string): void {
    for (const ws of this.wss.clients) {
      if (ws.readyState === 1) ws.send(msg);
    }
  }

  close(): Promise<void> {
    return new Promise((resolve) => {
      for (const ws of this.wss.clients) {
        ws.close();
      }
      this.server.close(() => resolve());
    });
  }
}
