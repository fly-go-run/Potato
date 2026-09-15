Rust 实现和报告已写好。临时补齐依赖后：

- 指定测试：5 passed
- 全量回归：182 passed，11 ignored

还需确认：是否允许修改 `potato-core/Cargo.toml` 和 `Cargo.lock`，增加 NFKC 所需的 `unicode-normalization` 直接依赖？这两个文件超出你指定的修改范围；未补依赖前，仓库正式命令无法编译。

[交付报告](docs/rfc/p2-cloud-memory-report.md) 已记录全部结果与待收尾项。未碰 Worker、GPUI、iOS 或他人改动。