import https from "https";
import fs from "fs";
import path from "path";
import { execSync } from "child_process";
import Koa from "koa";
import koaServe from "koa-static";
import { WebSocketServer } from "ws";

function generateCert(certDir: string): { key: string; cert: string } {
  const keyFile = path.join(certDir, "tur-wasm.key");
  const certFile = path.join(certDir, "tur-wasm.crt");

  if (fs.existsSync(keyFile) && fs.existsSync(certFile)) {
    return {
      key: fs.readFileSync(keyFile, "utf8"),
      cert: fs.readFileSync(certFile, "utf8"),
    };
  }

  fs.mkdirSync(certDir, { recursive: true });

  execSync(
    `openssl req -x509 -newkey rsa:2048 -keyout "${keyFile}" -out "${certFile}" -days 365 -nodes -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost"`,
    { stdio: "pipe" },
  );

  return {
    key: fs.readFileSync(keyFile, "utf8"),
    cert: fs.readFileSync(certFile, "utf8"),
  };
}

export class Server {
  private server: https.Server;
  private wss: WebSocketServer;
  private port: number;
  private host: string;

  constructor(dir: string, port: number, host: string = "0.0.0.0") {
    this.port = port;
    this.host = host;
    const app = new Koa();
    app.use(async (ctx, next) => {
      await next();
      ctx.set("Cache-Control", "no-cache, no-store, must-revalidate");
    });
    app.use(koaServe(dir));

    const certDir = path.join(dir, ".cert");
    const { key, cert } = generateCert(certDir);
    this.server = https.createServer({ key, cert }, app.callback());
    this.wss = new WebSocketServer({ server: this.server, path: "/__ws" });
  }

  start(): void {
    this.server.listen(this.port, this.host, () => {
      const addr = this.server.address();
      const address =
        typeof addr === "string"
          ? addr
          : `https://localhost:${addr!.port}`;
      console.log(`Serving at ${address}`);
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
