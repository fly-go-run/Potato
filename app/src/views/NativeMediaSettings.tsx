import { useEffect, useState } from "react";
import { apiJson, modelApi, type ProviderInfo } from "../lib/api";
import { useChatStore } from "../stores/chat";
import { detectNativeRuntime } from "../lib/nativeTransport";

interface MediaSettings {
  speech_provider_id: string;
  speech_model: string;
  image_provider_id: string;
  image_model: string;
}

interface DoubaoSettings { api_key: string; app_id: string; resource_id: string; enabled: boolean }

/** Additional connection fields inside the existing Settings page. */
export function NativeMediaSettings({ onSpeechSaved }: { onSpeechSaved: () => void }) {
  const [native, setNative] = useState(false);
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [media, setMedia] = useState<MediaSettings | null>(null);
  const [doubao, setDoubao] = useState<DoubaoSettings | null>(null);
  const [speechKey, setSpeechKey] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [legacy, setLegacy] = useState<{ working_dir: string; secret_dir: string } | null>(null);
  useEffect(() => {
    let mounted = true;
    void detectNativeRuntime().then(async (enabled) => {
      if (!enabled || !mounted) return;
      setNative(true);
      const [list, settings, speech, old] = await Promise.all([
        modelApi.list(), apiJson<MediaSettings>("/api/native/media-settings"),
        apiJson<DoubaoSettings>("/api/native/doubao-settings"),
        apiJson<{ working_dir: string; secret_dir: string }>("/api/native/legacy-settings"),
      ]);
      if (mounted) { setProviders(list); setMedia(settings); setDoubao(speech); setLegacy(old); }
    }).catch((e: Error) => { if (mounted) setNotice(e.message); });
    return () => { mounted = false; };
  }, []);
  const action = async (work: () => Promise<unknown>, message: string) => {
    setBusy(true); setNotice("");
    try { const result = await work(); setNotice(message || (typeof result === "string" ? result : "已完成")); } catch (e) { setNotice(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  };
  if (!native) return null;
  const field = "w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm text-ink";
  const button = "rounded-lg border border-line px-4 py-2 text-sm disabled:opacity-50";
  const choices = providers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>);
  return (
    <div className="space-y-6 text-ink">
        {doubao && <fieldset disabled={busy} className="space-y-4 rounded-xl border border-line p-5">
          <legend className="px-2 font-medium">豆包语音识别</legend>
          <label className="block space-y-1"><span>语音 API Key / Access Key（留空保留已保存的密钥）</span><input className={field} type="password" autoComplete="off" value={speechKey} onChange={(e) => setSpeechKey(e.target.value)} /></label>
          <label className="block space-y-1"><span>App ID（新版 API Key 留空）</span><input className={field} value={doubao.app_id} onChange={(e) => setDoubao({ ...doubao, app_id: e.target.value })} /></label>
          <label className="block space-y-1"><span>语音资源 ID</span><input className={field} value={doubao.resource_id} onChange={(e) => setDoubao({ ...doubao, resource_id: e.target.value })} /></label>
          <button className={button} onClick={() => void action(async () => { setDoubao(await apiJson<DoubaoSettings>("/api/native/doubao-settings", { method: "PUT", body: JSON.stringify({ app_id: doubao.app_id, resource_id: doubao.resource_id, ...(speechKey ? { api_key: speechKey } : {}) }) })); setSpeechKey(""); onSpeechSaved(); }, "豆包语音设置已保存")}>保存豆包设置</button>
        </fieldset>}
        {media && <fieldset disabled={busy} className="space-y-4 rounded-xl border border-line p-5">
          <legend className="px-2 font-medium">语音和图片</legend>
          <p className="text-sm text-ink-secondary">图片生成使用独立的图片接口。启用豆包时优先使用豆包流式识别；以下语音连接用于其他服务的录音后转写。</p>
          <label className="block space-y-1"><span>语音供应商</span><select className={field} value={media.speech_provider_id} onChange={(e) => setMedia({ ...media, speech_provider_id: e.target.value })}><option value="">未启用</option>{choices}</select></label>
          <label className="block space-y-1"><span>语音模型名称</span><input className={field} value={media.speech_model} onChange={(e) => setMedia({ ...media, speech_model: e.target.value })} /></label>
          <label className="block space-y-1"><span>图片供应商</span><select className={field} value={media.image_provider_id} onChange={(e) => setMedia({ ...media, image_provider_id: e.target.value })}><option value="">未启用</option>{choices}</select></label>
          <label className="block space-y-1"><span>图片模型名称</span><input className={field} value={media.image_model} onChange={(e) => setMedia({ ...media, image_model: e.target.value })} /></label>
          <button className={button} onClick={() => void action(() => apiJson("/api/native/media-settings", { method: "PUT", body: JSON.stringify(media) }), "语音和图片设置已保存")}>保存语音和图片设置</button>
        </fieldset>}
        {notice && <p role="status" className="rounded-lg border border-line p-3 text-sm">{notice}</p>}
        {legacy && <fieldset disabled={busy} className="space-y-4 rounded-xl border border-line p-5">
          <legend className="px-2 font-medium">导入旧版连接设置</legend>
          <p className="text-sm text-ink-secondary">读取旧供应商、豆包语音和图片插件的连接配置。已配置的连接不会覆盖，旧目录保持不变。</p>
          <label className="block space-y-1"><span>旧数据目录</span><input className={field} value={legacy.working_dir} onChange={(e) => setLegacy({ ...legacy, working_dir: e.target.value })} /></label>
          <label className="block space-y-1"><span>旧密钥目录</span><input className={field} value={legacy.secret_dir} onChange={(e) => setLegacy({ ...legacy, secret_dir: e.target.value })} /></label>
          <button className={button} onClick={() => void action(async () => {
            const result = await apiJson<{ providers_imported: number; speech_imported: boolean; image_imported: boolean; unsupported_providers: number }>("/api/native/legacy-settings", { method: "POST", body: JSON.stringify(legacy) });
            const [list, settings, speech] = await Promise.all([modelApi.list(), apiJson<MediaSettings>("/api/native/media-settings"), apiJson<DoubaoSettings>("/api/native/doubao-settings")]);
            setProviders(list); setMedia(settings); setDoubao(speech); onSpeechSaved();
            return `已导入 ${result.providers_imported} 个连接，语音${result.speech_imported ? "已导入" : "未更改"}，图片${result.image_imported ? "已导入" : "未更改"}${result.unsupported_providers ? `；${result.unsupported_providers} 项暂不兼容` : ""}`;
          }, "")}>导入连接设置</button>
        </fieldset>}
        <label className="block space-y-2 rounded-xl border border-line p-5">
          <span className="font-medium">导入旧会话</span>
          <p className="text-sm text-ink-secondary">选择一次性导出的会话 JSON。已有会话不会覆盖；旧附件文件需要另行迁移。</p>
          <input type="file" accept="application/json,.json" disabled={busy} onChange={(e) => {
            const file = e.target.files?.[0]; if (!file) return;
            void action(async () => {
              if (file.size > 50_000_000) throw new Error("导入文件不能超过 50 MB");
              const result = await apiJson<{ imported: number; skipped: number }>("/api/native/import-history", { method: "POST", body: await file.text() });
              await useChatStore.getState().refreshChats();
              return result;
            }, "会话导入完成，返回聊天列表查看");
            e.target.value = "";
          }} />
        </label>
    </div>
  );
}
