# Provider protocol audit — 2026-09-28

Sources consulted:
- OpenAI developer quickstart / Responses API: https://platform.openai.com/docs/quickstart/make-your-first-api-request
- OpenAI data controls: https://platform.openai.com/docs/models/default-usage-policies-by-endpoint
- Anthropic Messages API / platform docs: https://docs.anthropic.com/en/api/messages
- Google Gemini API: https://ai.google.dev/gemini-api/docs
- Gemini Interactions: https://ai.google.dev/gemini-api/docs/interactions
- Ollama chat API: https://docs.ollama.com/api/chat
- Ollama tool calling: https://docs.ollama.com/capabilities/tool-calling

No SDK source tree from these providers is vendored in v0.31. The provider crates are Phoenix-authored protocol adapters.
