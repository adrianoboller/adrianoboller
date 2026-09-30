-- PhxClaw v0.70 / native qualification evidence matrix
CREATE SCHEMA IF NOT EXISTS phxclaw;
CREATE TABLE IF NOT EXISTS phxclaw.native_verification_runs (
  run_uuid uuid PRIMARY KEY,
  gate_code text NOT NULL,
  sprint_code text,
  platform text NOT NULL,
  environment_fingerprint text NOT NULL,
  status text NOT NULL CHECK(status IN ('running','passed','failed','blocked')),
  evidence_sha256 text,
  artifact_uri text,
  details jsonb NOT NULL DEFAULT '{}'::jsonb,
  started_at timestamptz NOT NULL DEFAULT now(),
  completed_at timestamptz,
  CHECK ((status='running' AND completed_at IS NULL) OR (status<>'running' AND completed_at IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS idx_native_verification_gate ON phxclaw.native_verification_runs(gate_code,platform,started_at DESC);
CREATE TABLE IF NOT EXISTS phxclaw.native_gate_catalog (
  gate_code text PRIMARY KEY,
  sprint_code text,
  required boolean NOT NULL DEFAULT true,
  description text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO phxclaw.native_gate_catalog(gate_code,sprint_code,description) VALUES
 ('desktop_os_automation_e2e','F11','Keyboard, mouse, capture and governed shell on physical desktop'),
 ('real_stt_model_e2e','F12','whisper.cpp executable plus verified model transcription'),
 ('native_tauri_e2e','F14','Native Tauri desktop build and UI/live-event bridge'),
 ('channel_provider_credentialed_e2e','F21','Credentialed Telegram/Discord/Slack/WhatsApp/Teams delivery'),
 ('device_pairing_wss_keyring_multiplatform_e2e','F22','Physical WSS pairing and OS-keyring node execution')
ON CONFLICT(gate_code) DO UPDATE SET sprint_code=EXCLUDED.sprint_code,description=EXCLUDED.description;
