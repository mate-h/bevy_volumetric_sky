# Bevy Volumetric Sky

A volumetric sky plugin for Bevy based on the Unreal Engine Atmospheric Shader paper published by Sebastian Hillaire.

Running Native app:
```
cargo run
```

Running WASM Web app:
```
pnpm install
pnpm build:wasm
pnpm run dev
```

Note: this repository is still under development and is not yet ready for production use. Releasing it for early feedback.

Related work:
- https://github.com/mate-h/sky-model
- https://observablehq.com/@mateh/atmospheric-simulation
- https://github.com/Orillusion/orillusion/pull/444

References:
- https://github.com/sebh/UnrealEngineSkyAtmosphere
- https://github.com/ebruneton/precomputed_atmospheric_scattering