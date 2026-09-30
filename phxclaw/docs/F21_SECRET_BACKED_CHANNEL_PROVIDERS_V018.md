# F21 — Secret-backed Telegram / Discord Providers v0.18

Status: **review**.

Two REST providers are now implemented in `phxclaw-channel-providers`.

## Telegram

- `getMe` probe.
- `sendMessage` outbound text.
- token is resolved from F23 lease for `channel:telegram:probe` or `channel:telegram:send`.
- default origin: `https://api.telegram.org`.

## Discord

- `/users/@me` probe.
- `/channels/{id}/messages` outbound text.
- token is resolved from F23 lease for `channel:discord:probe` or `channel:discord:send`.
- default origin: `https://discord.com`.
- `allowed_mentions.parse=[]` by default.

## Security

- no token in PhxClaw plugin manifests.
- no token in PhxClaw CLI argv.
- no token in evidence/log result summaries.
- exact-origin allowlist required.
- HTTP denied by default; test-only local HTTP can be explicitly enabled through provider policy.
- providers are disabled by default in `config/channel-providers.json`.

## Not yet release-ready

Internet access is unavailable in the current build environment, so Telegram/Discord real-account E2E tests have not run. Those are explicit release gates.
