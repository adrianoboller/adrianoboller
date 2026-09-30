# External Capability Bridges — v0.12

The following capabilities are now declared as **private, disabled-by-default external bridges**:

- Create Image → `image.generate`
- Deep Research → `research.deep`
- Search → `web.search`
- Template Creator → `template.create`
- PDF → `pdf.manage`
- Spreadsheets → `spreadsheet.manage`
- Visualize → `visualize.render`
- OpenAI Platform → `openai.platform.manage`

They are intentionally not marked runtime-verified. A standalone PhxClaw process cannot impersonate ChatGPT-hosted plugins. Each bridge must receive an explicit provider/runtime connector, credentials via F23, policy, and E2E evidence before enablement.
