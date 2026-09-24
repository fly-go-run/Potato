import { DurableObject } from 'cloudflare:workers';
import type { ModelSettings } from './cloud-models.ts';

/** One global instance holds the admin-edited model list. */
export class CloudModelSettings extends DurableObject<Env> {
  async read(): Promise<ModelSettings | null> {
    return (await this.ctx.storage.get<ModelSettings>('settings')) ?? null;
  }
  /** Compare-and-set on revision so two phones cannot silently overwrite each other. */
  async write(next: Omit<ModelSettings, 'revision'>, revision: number): Promise<ModelSettings | null> {
    const current = await this.ctx.storage.get<ModelSettings>('settings');
    if ((current?.revision ?? 0) !== revision) return null;
    const value: ModelSettings = { ...next, revision: revision + 1 };
    await this.ctx.storage.put('settings', value);
    return value;
  }
}
