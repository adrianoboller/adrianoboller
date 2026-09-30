import { Plugin, Notice } from 'obsidian';
async function sha256(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest)).map(b => b.toString(16).padStart(2, '0')).join('');
}
export default class PhxClawBridge extends Plugin {
  async onload() {
    this.addCommand({ id: 'phxclaw-export-active-note', name: 'Stage active note for PhxClaw', callback: async () => {
      const f = this.app.workspace.getActiveFile(); if (!f) { new Notice('No active note'); return; }
      const content = await this.app.vault.read(f);
      const payload = { schema_version: 'phxclaw.obsidian.exchange.v1', path: f.path, content, content_sha256: await sha256(content), trust: 'unverified_context', exported_at: new Date().toISOString() };
      const dir = '.phxclaw/inbox'; try { await this.app.vault.adapter.mkdir(dir); } catch (_e) {}
      const safe = f.path.replace(/[^a-zA-Z0-9._-]/g, '_');
      await this.app.vault.adapter.write(`${dir}/${safe}.json`, JSON.stringify(payload, null, 2));
      new Notice('Staged for PhxClaw');
    }});
  }
}
