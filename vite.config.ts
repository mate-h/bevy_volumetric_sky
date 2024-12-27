import { createLogger, defineConfig } from "vite";
import { spawn } from "child_process";
import path from "path";
import fs from "fs";

export default defineConfig({
  plugins: [
    {
      name: "copy-assets",
      generateBundle() {
        const files = fs.readdirSync("assets", { recursive: true });
        for (const file of files) {
          const p = "assets/" + file;
          if (fs.statSync(p).isDirectory()) continue;
          this.emitFile({
            type: "asset",
            fileName: p,
            source: fs.readFileSync(p),
          });
        }
      },
    },
    {
      name: "wgsl-hmr",
      handleHotUpdate({ file, server }) {
        if (file.endsWith(".wgsl")) {
          server.ws.send({
            type: "custom",
            event: "wgsl-update",
            data: { file: file.replace(process.cwd() + "/assets/", "") },
          });
          return [];
        }
      },
    },
    {
      name: "rust-wasm",
      configureServer(server) {
        server.watcher.add(path.resolve(__dirname, "src/**/*.rs"));
        const logger = createLogger("info", {
          prefix: "[rust-wasm]",
        });
        server.watcher.on("change", (file) => {
          if (file.endsWith(".rs") && file.includes(path.join("src", ""))) {
            logger.info("rust file changed. rebuilding wasm...", {
              timestamp: true,
            });
            spawn("pnpm", ["run", "build:wasm"], { stdio: "inherit" });
          }
        });
      },
    },
  ],
});
