# 任务：图标尺寸/描边机械归格（design/crispness-final 分支）

你是执行工程师。规则已由设计评审定死（见 icon-audit-grok.md §字重/
尺寸），本任务纯机械执行，**不做任何审美再判断**。范围 `app/src`。

## 归格规则

1. **尺寸档**（lucide `size=`）：
   - 导航/工具栏级（侧栏导航、顶栏、composer 控制行、页头动作钮）
     → 16
   - 行内 chrome（工具行/列表行内图标、chevron、菜单项图标、
     disclosure）→ 14
   - 微型（行尾状态、11-12px 文字旁）→ 12
   - 映射：13→14、15→16、17→16、15 行内→14;11→12。
     18/20（composer +/Mic）→ 归 16? **否**:composer 的 Plus 20 与
     Mic 18 保持现尺寸(它们是该行的主锚点),只改描边。
   - 22/24/28(空态大图形、品牌位)不动。
2. **描边档**（`strokeWidth`）：
   - size≥16 → 1.75
   - size 14 → 1.8
   - size ≤12 → 1.8
   - 例外:发送键 ArrowUp 2.4 **保留**;录音停止 Square fill 不涉及。
   - 现散布的 1.9(Plus/Mic)统一到 1.75。
3. **不许改**：图标选型(形状)、颜色类、布局类;FileToolCard 行内
   12px 双态一致性注释所述的刻意设计(尺寸 12 保留,只补 1.8)。

## 验收

- `npm test`、`tsc -b`、`npm run build` 全绿。
- 截图 QA:dev server 起 5307 端口
  (QWENPAW_DEV_BACKEND=http://127.0.0.1:61714 npm run dev --
  --port 5307 --strictPort),用 __qa.html 管线截浅色首页/会话页,
  与改前对照确认无布局破坏(图标溢出、行高变化)。
- 完成报告 app/docs/icon-normalize-report.md:改动统计表
  (按尺寸档几处、描边几处)、跳过项及理由。
- commit 一个,信息英文。遇到规则覆盖不到的场景:保持现状并记录,
  不要自行发明档位。
