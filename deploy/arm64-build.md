# ARM64 Build Guide

Life Engine Core supports ARM64 (aarch64) for Raspberry Pi and other ARM64 Linux systems. There are two approaches: Docker multi-arch builds (recommended) and native cross-compilation.

## Docker Multi-Arch Build (Recommended)

The existing Dockerfile supports ARM64 natively via Docker buildx. Both base images (`rust:1.85-alpine` and `alpine:3.20`) provide ARM64 variants.

Build for ARM64 Linux:

```bash
docker buildx build --platform linux/arm64 -t life-engine-core:arm64 -f deploy/Dockerfile .
```

Build for both architectures simultaneously:

```bash
docker buildx build --platform linux/amd64,linux/arm64 \
  -t life-engine-core:latest -f deploy/Dockerfile .
```

Run the ARM64 image (works on ARM64 hosts or via QEMU on x86_64):

```bash
docker run --platform linux/arm64 -p 3750:3750 \
  -v le-data:/data -v le-plugins:/plugins -v le-workflows:/workflows \
  life-engine-core:arm64
```

## Native Cross-Compilation

For building outside Docker, use the `cross` tool (Docker-based cross-compilation):

```bash
cargo install cross --git https://github.com/cross-rs/cross
cross build --release --package life-engine-core --target aarch64-unknown-linux-gnu
```

The resulting binary is at `target/aarch64-unknown-linux-gnu/release/life-engine-core`.

Alternatively, install a system cross-linker and use cargo directly:

```bash
# macOS (Homebrew)
brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu
cargo build --release --package life-engine-core --target aarch64-unknown-linux-gnu

# Ubuntu/Debian
sudo apt install gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
cargo build --release --package life-engine-core --target aarch64-unknown-linux-gnu
```

The linker configuration is in `.cargo/config.toml` at the workspace root.

## Verification Checklist

After building, verify the ARM64 binary:

- **Build succeeds** — `cargo build` or `docker buildx build` exits 0
- **Binary runs** — the binary starts on an ARM64 host or via QEMU emulation
- **Health check responds** — `GET /api/system/health` returns 200
- **Memory usage** — idle memory stays under 128 MB with no plugins loaded
- **SQLCipher works** — encrypted database create, write, read round-trip succeeds (bundled-sqlcipher compiles OpenSSL-free SQLCipher from source, so no runtime dependency)

## Notes

- The `rusqlite` dependency uses `bundled-sqlcipher`, which compiles SQLCipher from C source during the build. This works on ARM64 without modification because the bundled build system detects the target architecture automatically.
- The Alpine-based Docker image produces a fully static musl binary — no runtime shared library dependencies.
- On Raspberry Pi 4 (1 GB+ RAM), the build itself requires Docker or cross-compilation from a more powerful machine. The resulting binary runs comfortably within the 128 MB idle budget.
