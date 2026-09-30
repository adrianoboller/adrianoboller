# PhxClaw v0.10 — olmOCR Community Plugin

## Decision

Integrate olmOCR as a **community process plugin**, not as a new microkernel dependency.

Reasons:

- it is Python/GPU-heavy and should not enlarge the Rust core;
- it can evolve independently from PhxClaw;
- it is a good demonstration that community plugins may be implemented in languages other than Rust as long as they obey the process contract;
- it preserves deny-by-default capability routing, sandboxing, evidence and rollback.

## Capability model

```text
Document / Image
      │
      ▼
OCR Router / Agent policy
      │
      ├── ocr.read              → built-in Tesseract path
      │
      └── ocr.olmocr.extract    → community olmOCR plugin
                                      │
                                      ▼
                              Phoenix Process Protocol
                                      │
                                      ▼
                                  olmOCR CLI
                                      │
                         ┌────────────┴────────────┐
                         ▼                         ▼
                  local preloaded model      remote vLLM/OpenAI-compatible
                    network denied             future proxy sandbox only
```

## Provider-selection policy

Use built-in OCR for simple screenshots, labels, forms with clear printed text, or low-cost deterministic extraction.

Use olmOCR when the input contains complex reading order, tables, equations, handwriting, multi-column content, figures/insets, or when Markdown reconstruction is more valuable than raw text.

The router must not silently switch to a cloud endpoint. Remote mode needs an explicit approved origin and network policy.

## Evidence contract

The adapter returns:

- job UUIDv7;
- source SHA-256;
- output SHA-256;
- provider and model metadata;
- local output artifact path;
- elapsed time;
- source repository/license hints.

The Extension Host then records invocation UUID, correlation UUID, capability, plugin UUID and result in the existing Evidence Ledger.

## Security constraints

- input path confined to `var/documents`;
- output confined to `var/olmocr`;
- supported extensions only: PDF/PNG/JPEG;
- no shell interpolation;
- local mode refuses hidden model downloads;
- remote mode requires exact-origin allowlist;
- authenticated remote mode blocked until Secret Broker integration;
- plugin starts disabled and capability route is explicit;
- output returned through the process frame is size-bounded.

## Runtime status levels

`adapter_ready` means the PhxClaw wrapper is functional.

`backend_available` means the upstream `olmocr` executable/module is installed.

`local_model_exists` means a pre-provisioned model path exists.

A production gate requires all applicable items plus a real document E2E result. Source/static checks alone are not sufficient.
