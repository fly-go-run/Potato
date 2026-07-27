# Phase 6 视觉修正（执行者：Codex）— 对标附图

附图 1 = ChatGPT 桌面版 Plugins 页；附图 2 = Codex Desktop Skills 页。
把 `/skills` 页面重塑为附图的视觉语言。只改视觉/布局，功能逻辑不动。
约束不变（语义色类、i18n、不新增依赖、不动 tokens.css）。逐条执行：

1. **修 bug**：技能行右侧 Switch 与「已启用/已停用」文字重叠裁切。
   删掉文字标签，只留 Switch（附图 1 就是纯开关）。切换中的行内 loading 保留。
2. **Tab 行重做**：去掉现在通栏两段式 segmented。改为附图 1 式紧凑 pill tab 靠左：
   「技能 17」「插件 N」（数量实时来自已加载数据，激活 tab 底色 bg-line/60 圆角 pill，
   非激活纯文字 ink-secondary）；搜索框缩窄（max-w-xs）放同一行右侧。
3. **列表去卡片化**：行不再用独立描边卡片。整列表一个容器（rounded-lg border border-line），
   行间用 divide-y divide-line，行 hover bg-line/30。行内布局保持：
   emoji 圆角方块 + 名称（+版本淡显）+ 单行描述 + Switch。
4. 名称后加来源淡显标签（数据里的 source/installed_from，如 builtin/hub），
   样式同附图 1 的 "claude-plugins-official"（text-xs text-ink-muted）。
5. 插件 tab 应用同样的列表样式与 tab 行。
6. 「添加」按钮保持右上，但降为次级样式（border 描边，非实心 accent）——
   附图 1 顶部没有大主按钮，页面主体是列表本身。
7. `npm test` + `npm run build` 通过即交付；无需真实联调（纯视觉改动）；
   报告追加到 phase6-report.md 末尾（≤15 行）。
