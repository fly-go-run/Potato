# 任务:对抗性 review 设置窗口信息架构重做(W-S1/S2/S3)

你是严苛的代码审查者,目标是挑战并推翻下列实现,找出真实缺陷。只审查,**不修改任何文件**。

## 本轮改动

1. `app/src/App.tsx`:设置改为 background-location 覆盖层(应用内打开时背景页面保持挂载;直链 /settings 退化整页)。入口带 state:`Sidebar.tsx`、`ModelPicker.tsx`、`Composer.tsx`。
2. `app/src/views/SettingsView.tsx`:**整文件重写**。五分区 → 两分区(模型与服务商 / 通用);供应商 master-detail(列表排序:已配置>本地>其余;详情=连接组[key 显式保存/清除/测试连接] + 模型管理 inline[发现/增删/当前徽标]);新建自定义供应商表单(名称 slug 化为 id → POST custom-providers → 可选 key → 进详情);删除 blur 自动保存;当前模型改只读展示。
3. `app/src/lib/api.ts`:新增 `modelApi.testProvider`(POST /{id}/test,可带未保存 key/url)、`modelApi.createCustomProvider`。
4. i18n 新增若干 key(注意检查有无再重复)。

后端契约(勿改):`src/qwenpaw/app/routers/providers.py`(config PUT 部分更新、api_key:"" 清空、/test、/custom-providers)、`provider.update_config`。

## 重点挑战方向

1. **路由/状态**:background-location 在 HashRouter 下的边界(刷新后 state 丢失?设置内跳 /skills 后返回?closePanel navigate(-1) 的历史栈;desktop 壳 ?desktop=1 初始导航)。
2. **重写丢功能**:对照 git 里旧版 SettingsView(用 `git diff` 不可用,app/ untracked;可看我留的旧行为线索或自行判断),有没有旧能力被弄丢且未在报告里声明的(已知有意变更:添加模型不再自动激活、设置不再提供切换模型入口)。
3. **供应商流程**:创建 id slug 冲突(同名两次创建)、创建后 configure key 失败的半完成态、删除供应商后 activeModel 兜底逻辑、清除 key 后 providerConfigured 状态、测试连接携带 url 变更的判断。
4. **状态残留**:detail 打开时 providers 刷新导致 detailProvider 查不到(discover 后 id 仍在,ok?)、keyDraft/urlDraft 与 provider 刷新的同步、错误横幅的清理时机。
5. **可用性**:必填校验、busy 态覆盖、键盘提交。

不要报:纯风格、i18n 复数、旧版遗留未用的 i18n key。

## 输出

P0/P1/P2 分级,每条:文件:行号 + 触发场景 + 一句话修法。最后一行总评:能不能上。
