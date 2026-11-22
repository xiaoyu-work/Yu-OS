# Model Module

## Purpose

`model/` owns local model package/runtime selection and task-specific model
implementations outside the conversational provider layer.

## Responsibilities

- Resolve and validate installed model engine packages.
- Build embedding and other task runtimes.
- Keep architecture/runtime compatibility explicit.
- Expose model tasks to core and CLI callers.

## Key Files

| Path | Role |
| --- | --- |
| `tasks/` | Embedding and task-specific model interfaces/implementations |
| `engines/` | llama.cpp, ONNX Runtime, ORT GenAI backends |
| `compat.rs` | Host/model compatibility |
| `import.rs` | Model import lifecycle |
| `mod.rs` | CLI/task dispatch and exports |
| `local_embedding.rs` | Versioned fixed-Qwen3 CPU pipe for installed file indexers and Root-produced non-root owner helpers |
| `tasks/decision.rs` | Bounded decision inference over the owner's private System One socket |
| `tasks/stt/azure.rs` | Azure OpenAI deployment audio transport, shared by model CLI and Agent media |

## Dependencies

Task callers depend on model-task interfaces, not engine implementation
details. Engine packages are validated before loading; local failure must not
silently fall back to fake embeddings or success-shaped output.

The file-index pipe uses only the installed Qwen3 model slot and managed local
engines, independent of conversational/cloud embed configuration. It carries
no file path, capability, provider credential or executable selector. Root
parentage permits only fixed non-root CPU inference; Workspace file authority
remains at the caller's broker boundary.

The [decision task](../../../docs/decision-model.md) is separately configured
and never sends state to a remote provider. Callers own any effects selected
from its answers and must reauthorize them through their existing tools.

ORT GenAI preloads and retains the registered ONNX Runtime dependency before
loading its native initializer. Both packages are required; installing GenAI
alone must return an actionable missing-dependency error, not let a foreign
exception abort the process. The image's validated pair is declared in
`engine_pkg/mod.rs`. File inference uses a 2048-token ceiling; the general
Qwen3 embedding task retains its existing separate token limit.

## Azure OpenAI speech to text

Deploy a Whisper or GPT transcription model in Azure OpenAI and use its
**deployment name**, not the underlying model ID. Use the resource's HTTPS
endpoint from Azure, not the deployment URL or an Azure AI Speech endpoint.

```bash
yu agent setup stt apply --provider azure --model my-transcriber \
  --base-url https://example-resource.openai.azure.com/ \
  --api-key-env AZURE_OPENAI_API_KEY
yu agent setup stt status --json
yu model transcribe audio.wav
```

For private credential storage, replace `--api-key-env AZURE_OPENAI_API_KEY`
with `--api-key-stdin` and supply the key over stdin. `--api-version` is
optional and defaults to `2024-10-21`. Override it if your deployment needs
a different deployment-scoped audio API version. The setup status validates
configuration and key availability without uploading audio or probing Azure.

The Azure provider uses the configured resource and deployment with `api-key`
auth; it does not forward custom headers or follow redirects. Transcription
preserves the input language. `yu model translate` targets the Azure translation
route, which requires a Whisper deployment supporting translation to English.
Use `yu model transcribe` for GPT transcription deployments.

## Tests

```bash
cargo test -p yu model:: -- --test-threads=1
```
