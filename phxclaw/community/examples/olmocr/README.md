# PhxClaw olmOCR Plugin

Community-style process adapter for [AllenAI olmOCR](https://github.com/allenai/olmocr).

## Why this plugin exists

The built-in `ocr.read` provider remains suitable for lightweight deterministic OCR with Tesseract. This plugin adds a VLM-based path for documents where reading order, tables, equations, handwriting, multi-column layouts, headers/footers, or complex page structure matter.

The plugin does **not** vendor olmOCR or model weights. Install the upstream dependency separately, then sign/install this adapter through the PhxClaw community plugin flow.

## Capabilities

- `ocr.olmocr.health`
- `ocr.olmocr.extract`
- `ocr.olmocr.batch`

Inputs: PDF, PNG, JPG/JPEG.

Outputs: Markdown or flattened text plus SHA-256 hashes and job metadata.

## Local GPU mode

PhxClaw intentionally does not allow implicit model downloads from inside the sandbox. Configure a model that is already available locally:

```bash
export PHXCLAW_OLMOCR_BIN=/usr/local/bin/olmocr
export PHXCLAW_OLMOCR_LOCAL_MODEL=/opt/models/olmOCR-2-7B-1025-FP8
```

Then invoke `ocr.olmocr.extract` with a path under `var/documents/`.

## Remote inference mode

The adapter source supports an external OpenAI-compatible/vLLM server, but the default manifest is `network=deny`. Remote inference must stay disabled until PhxClaw runs the plugin under a network-proxy sandbox with an exact-origin allowlist.

Authenticated remote inference is also deliberately blocked by the adapter until the Secret & Credential Broker can inject the API key without exposing it in a command line.

## Example process envelope

```json
{
  "protocol": "phxclaw-process-v1",
  "message_uuid": "0199a6e0-0000-7000-8000-000000000001",
  "correlation_uuid": null,
  "kind": "execute",
  "sent_at": "2026-09-28T01:00:00Z",
  "payload": {
    "capability": "ocr.olmocr.extract",
    "payload": {
      "input_path": "invoice.pdf",
      "mode": "local",
      "output_format": "markdown",
      "include_content": true
    }
  }
}
```

## Installation lifecycle

1. Install upstream olmOCR in a clean Python 3.11 environment or approved container.
2. Pin the upstream release/commit and model version.
3. Run `python3 scripts/test_olmocr_plugin.py`.
4. Replace the integrity placeholders in `phxclaw.plugin.template.json` by signing with the publisher's Ed25519 key.
5. Package with `scripts/plugin_pack.py`.
6. Verify SBOM, license, SHA-256 and health.
7. Install **disabled**.
8. Pin capability routes explicitly.
9. Enable only after a real OCR E2E test passes on the target host.

## Upstream requirements

The upstream project currently documents Python 3.11+, Poppler/fonts for PDF rendering, a lightweight remote-inference install, and a heavier local GPU install. Review the upstream README at deployment time because GPU/CUDA/package requirements change.
