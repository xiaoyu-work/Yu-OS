# Yu OS SDK

Official multi-language App SDK for [Yu OS](https://github.com/xiaoyu-work/Yu-OS) ([GitHub Releases](https://github.com/xiaoyu-work/Yu-OS/releases/tag/yuos-sdk-v0.1.0.dev2026112401)).

Call the user's configured models, expose MCP tools, and use OS services from your App.

**Start:** [Quickstart (`quickstart.html`)](quickstart.html) · [API Reference (`api.html`)](api.html) · [Linux App Integration (`linux-app-integration.html`)](linux-app-integration.html) · [For coding agents (`llms.txt`)](llms.txt)

## Download & Install (`v0.1.0.dev2026112401`)

- **Python (3.10+)**:
  ```sh
  python3 -m pip install https://github.com/xiaoyu-work/Yu-OS/releases/download/yuos-sdk-v0.1.0.dev2026112401/yuos_sdk-0.1.0.dev2026112401-py3-none-any.whl
  ```
- **Node.js / TypeScript (18+)**:
  ```sh
  npm install https://github.com/xiaoyu-work/Yu-OS/releases/download/yuos-sdk-v0.1.0.dev2026112401/yuos-sdk-node-0.1.0.dev2026112401.tgz
  ```
- **Go (1.21+)**:
  ```sh
  go get github.com/xiaoyu-work/Yu-OS/yuos-sdk/go@v0.1.0-dev.2026112401
  ```
- **Rust (Edition 2021, in `Cargo.toml`)**:
  ```toml
  [dependencies]
  yuos-sdk = { git = "https://github.com/xiaoyu-work/Yu-OS.git", tag = "yuos-sdk-v0.1.0.dev2026112401" }
  ```

**Per-Language READMEs:** [Python](python/README.md#install) · [Node.js](node/README.md#install) · [Go](go/README.md#install) · [Rust](rust/README.md#install)

**Reference:** [Permissions & Overview (`reference.html`)](reference.html) · [App development](../docs/app-development.md) · [Wire protocol](wire/v1/README.md)

**MCP handlers:** [Generate manifest-based types for TypeScript or Rust](../docs/app-development.md#typed-mcp-handlers)
