# Bevy WebGPU

Bevy WebGPU is a template repository for getting started with [Bevy game engine](https://bevyengine.org/) for web game development using WebGPU. Efficient hot reloading of WGSL files with Vite dev server, as well as recompiling Rust code with page refresh.

Demo: https://bevy-webgpu.vercel.app/

Quickstart:

```bash
pnpx degit mate-h/bevy-webgpu my-project
cd my-project
pnpm i
pnpm run build:wasm
pnpm run dev
```

Features:
- WebGPU support with Bevy version 0.15 engine
- TypeScript support with WASM bindings to Rust
- Hot reloading of WGSL files
- Recompiling Rust code with page refresh
- Instant reloading of shaders without refreshing the page
- Support for multiple examples
- Easy to understand and modify template
- Egui support for debugging

No dependencies besides:
- Bevy
- Egui
- Vite
- Rust
- Node
- WASM
- WebGPU 

## Recommended setup

Use rust-analyzer for Rust development, as well as wgsl-analyzer for WGSL linting. Fork of the wgsl-analyzer is available [here](https://github.com/mate-h/wgsl-analyzer). This fork includes support for Bevy shaders that use `#import` directives as opposed to `#include`.

It is also recommended to use [Mise en place](https://mise.jdx.dev/getting-started.html) for installing the Rust toolchain and other dependencies.

```bash
mise use -g rust
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

VSCode plugins:
- [Rust Analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
- [WGSL Analyzer](https://marketplace.visualstudio.com/items?itemName=wgsl-analyzer.wgsl-analyzer)

In order to properly lint Bevy shaders, you need to add the following VSCode configuration.
This is described in [this issue](https://github.com/bevyengine/bevy/issues/5561).

VSCode configuration:
```jsonc
{
  "rust-analyzer.server.path": "~/.local/share/mise/installs/rust/latest/bin/rust-analyzer",
  "rust-analyzer.server.extraEnv": {
    "RUSTUP_TOOLCHAIN": "stable"
  },
  "wgsl-analyzer.server.path": "/your/path/to/wgsl-analyzer/target/release/wgsl_analyzer",
  "wgsl-analyzer.customImports": {
    "bevy_pbr::mesh_view_bindings": "file:///path/to/bevy/crates/bevy_pbr/src/mesh_view_bindings.wgsl",
    // Add more imports here. To obtain the full list, see the linked GitHub issue.
  },
  "wgsl-analyzer.preprocessor.shaderDefs": [
    "VERTEX_UVS",
    "VERTEX_TANGENTS",
    "VERTEX_COLORS",
    "SKINNED",
    "STANDARDMATERIAL_NORMAL_MAP",
  ],
}
```