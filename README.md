# vRain_rs 使用说明

本目录为 vRain 的 Rust 实现，对应 Perl 版 `vrain.pl` 的功能。以下说明按常见流程给出。

## 前置条件

-   安装 Rust stable（`cargo` 可用）。
-   准备好素材目录（与 Perl 版兼容）：
    -   `books/<book_id>/book.cfg`
    -   `books/<book_id>/text/*.txt`
    -   可选封面：`books/<book_id>/cover.jpg|cover.png`
    -   画布配置与背景：`canvas/<canvas_id>.cfg`，`canvas/<canvas_id>.jpg|png`
    -   字体：`fonts/…`（配置里引用）
    -   数字映射：`db/num2zh_jid.txt`
-   可选压缩：安装 `gs`（Ghostscript）。未安装则自动跳过压缩。

## 运行

在 `vrain_rs` 目录执行：

```bash
cargo run --release -- \
  -b <book_id> \
  -f <from_entry> \
  -t <to_entry> \
  --books-dir ../books \
  --canvas-dir ../canvas \
  --fonts-dir ../fonts \
  --db-dir ../db \
  [-c  # 生成后尝试压缩，需要 gs]
  [-v  # 打印排版日志]
  [--test-pages <n>  # 仅排版 n 页用于调试]
```

示例（排版第 1-2 篇文本）：

```bash
cargo run --release -- -b shiji -f 1 -t 2 --books-dir ../books --canvas-dir ../canvas --fonts-dir ../fonts --db-dir ../db -c
```

输出文件路径：`books/<book_id>/《{标题}》文本{from}至{to}.pdf`，压缩后为 `…_compressed.pdf`。

仅生成背景图（替代 Perl 背景脚本）：读取 `books/<book_id>/book.cfg` 中的 `canvas_id`，加载对应 `canvas/<id>.cfg`，在缺少背景图时生成竹简/宣纸风格背景到 `canvas/<id>.jpg`（或用 `--bg-output` 指定路径），生成后立即退出：

```bash
cargo run --release -- -b <book_id> --generate-bg \
  --books-dir ../books --canvas-dir ../canvas
# 可选：--bg-output <自定义输出路径>
```

## 叠加印章（可选）

Rust 版内置了与 Perl `addyins.pl` 类似的盖章流程：

1. 在目标书籍目录（即输出 PDF 同级，例如 `books/<book_id>/`）放置 `yins.cfg`。
2. 在同级目录下创建 `yins/` 子目录，放置印章图片（支持 PNG/JPEG）。
3. `yins.cfg` 每一行格式：

    ```
    <pdf名或*>|<页码>,<列起始>,<行起始>,<占用列数>|<印章文件名>
    ```

    - `<pdf名>` 不含扩展名；用 `*` 可匹配任意输出（便于复用规则）。
    - 列/行起始均从 1 开始，对应 `canvas` 配置的版心网格；占用列数用于控制印章宽度（随列宽缩放，等比例变换）。

4. 运行 Rust 渲染流程，印章会自动按 `yins.cfg` 规则叠加到对应页、对应位置。规则格式错误或文件缺失时会打印警告并跳过，不影响 PDF 生成。

## 背景生成（缺图兜底）

如果 `canvas/<canvas_id>.jpg|png` 不存在，Rust 版会自动按画布配置生成一张“竹简/宣纸”风格的背景图（噪声、竖纹、绑带）。无需再手工运行 `bamboo.pl`，直接执行排版命令即可。

## 示例

Rust 版排版效果示意（使用仓库内示例素材）：

![示例页面](./images/010.png)
![示例近景](./images/014.png)

## 功能对照要点

-   支持简繁兜底（`try_st` 配置）。
-   书名号侧边波浪线（启用 `book_line_flag`）。
-   批注双排、非占位/旋转标点、中文页码、封面作者/背景/封面图片。
-   MultiRows 多栏模式：`multirows_enabled`/`multirows_horizontal_layout`/`multirows_count` 与 Perl 行为一致，支持 `^` 跳栏控制符。
-   `%` 强制分页、`$` 半页跳转、`&` 跳到末列，`《》《` 开关书名侧线，`【】` 批注。
-   多栏样例：可直接使用 `canvas` 中的多栏配置及 `books_mr` 目录的示例书籍，Rust 版已完整支持。

## 开启多栏模式

1. 画布（`canvas/<id>.cfg`）启用多栏：

```
if_multirows=1
multirows_num=<栏数>      # 如 5
multirows_linewidth=<线宽> # 可选，分栏线宽度
```

2. 书籍（`books/<id>/book.cfg` 或 `books_mr/<id>/book.cfg`）指定横向分带布局：

```
multirows_horizontal_layout=1  # 1=横向分带（与 Perl 多栏一致）
```

3. 运行时无需额外参数，照常 `cargo run …`。文本中的 `^` 控制符可用于跨带跳转。

## 常见问题

-   字体找不到：确认 `book.cfg` 中字体文件存在于 `fonts` 目录。
-   `gs` 不存在：压缩自动跳过，安装后再加 `-c`。
-   对齐差异：用 `--test-pages` 跑少量页与 Perl 输出对比，重点检查标点、批注跨页和多栏跳转。可在 `book.cfg` 或 `canvas` 参数微调。
