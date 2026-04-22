import { Command } from "commander";
import { buildWasm } from "./build";
import { serveAction } from "./actions/serve";

const program = new Command();
program
  .name("tur-wasm-cli")
  .description("CLI for building and serving tur wasm demos")
  .version("0.1.0");

program
  .command("build")
  .description("Build the tur-wasm package with wasm-pack")
  .action(() => {
    buildWasm();
  });

program
  .command("serve")
  .description("Build wasm and serve a JS demo app")
  .argument("<jsFile>", "Path to the JS bundle file to serve")
  .option("-p, --port <number>", "Port to serve on", "11223")
  .option("--no-build", "Skip wasm build step")
  .action(serveAction);

program.parse();
