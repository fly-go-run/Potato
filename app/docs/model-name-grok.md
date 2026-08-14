# 模型名产品化（挑战 Claude 草案）

读过 `ModelPicker.tsx`、`api.ts` 的 `ActiveModel` / `ModelInfo`、`SettingsView` 当前模型行。先讨论不改。

**现状。** `activeModel.active_llm` 只有 `{provider_id, model}`。chip（216）和设置当前行（1051）直接打 raw id；providers 未回时 chip 也查不到 `name`，必须能单靠 id 出产品名。菜单已按 `provider.name` 分组，行用 `item.name || item.id`（288）。后端 `ModelInfo.name` 已在，内置文案还打架（`DeepSeek-V4 Pro` / `DeepSeek V4 Flash`）。洞在消费，不在缺字段。新层不要绕过 `name` 再美化一遍；菜单只处理 `name == id` 的行，禁止双重加工。对照 `skillPresentation.ts`：白名单出产品名，raw 留 tooltip / 管理。

**删厂牌。** Claude 的「独特专名 + 通用厂牌才剥」不可执行。flash / pro / coder / mini 像词但不是身份；terra / sol 才是。能判别就等于白名单。**只有白名单允许删厂牌、重排词序**（`gpt-5.6-terra` → Terra 5.6）。兜底永不删首段。gpt 不是「通用词就可剥」：`gpt-5.6` 仍是 GPT 5.6，不是 5.6。DeepSeek 留下。优先序：`ModelInfo.name`（且 ≠ id）→ 白名单 → 兜底。设置模型管理保持 raw。

**兜底。**

```
pretty(id):
  if name && name != id: return name
  if whitelist[id]: return it
  s = last path segment                 # org/model
  s = strip /-\d{8}$/ and /-\d{4}-\d{2}-\d{2}$/
  keep preview|latest|beta|alpha|exp as last word
  if s has CJK: replace [-_]+ with space;
    Title-Case Latin tokens only; return
  tokens = split [-_:.]+
  never drop tokens[0]
  glue letter+digit (qwen3, v4 → Qwen3, V4)
  ACRONYM={gpt,glm,llm} uppercase; else Title Case
  join spaces
```

日期扔（留给 tooltip）。`preview` / `latest` 留，是档不是装饰。中文名禁止整串 Title Case。

**同名。** 菜单已有组头，行上不要再挂供应商副行。chip 仍单名；title 已是 `provider_id / model · 档`。设置当前行已有供应商副行。两家 `grok-4.6` 靠组头区分，不靠 chip。

**「高」。** 保持两段：名 ink、档 tertiary、`shrink-0`。不要拼成「Terra 5.6 高」——档会吃进品名，截断会切掉档。不加间隔点。
