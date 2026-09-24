import SwiftUI

struct DocumentPanel: View {
    @ScaledMetric(relativeTo: .title2) private var titleSize = 26.0
    @Binding var draft: WorkingDraft
    @Binding var expanded: Bool
    var keyboardVisible: Bool
    var close: () -> Void
    var share: (String) -> Void
    @State private var copied = false
    @State private var editing = false
    @State private var showingHistory = false
    @State private var editText = ""
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        VStack(spacing: 0) {
            Capsule().fill(Palette.line).frame(width: 38, height: 5).padding(.top, 10).padding(.bottom, 5).accessibilityHidden(true)
            HStack(spacing: 12) {
                Image(systemName: "doc").font(.title2).frame(width: 42, height: 42).background(Palette.canvas, in: RoundedRectangle(cornerRadius: 12))
                Text(L10n.tr("工作文稿")).font(.headline)
                Spacer(minLength: 0)
                IconButton(symbol: "square.and.pencil", label: L10n.tr("编辑文稿"), id: "edit-document") { editText = draft.markdown; editing = true }
                IconButton(symbol: expanded ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right", label: expanded ? L10n.tr("收起文稿") : L10n.tr("展开文稿"), id: "expand-document") {
                    withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.22)) { expanded.toggle() }
                }
                IconButton(symbol: "xmark", label: L10n.tr("关闭文稿"), id: "close-document", action: close)
            }.dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 16).padding(.bottom, 6)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    if let markdown = draft.customMarkdown {
                        MarkdownContent(text: markdown, toggleChecklist: { ordinal in draft.toggleMarkdownCheckbox(ordinal) })
                    } else {
                        Text(draft.title).font(.system(size: titleSize, weight: .bold)).fixedSize(horizontal: false, vertical: true).padding(.bottom, 10)
                        Text(L10n.tr("周末计划 · 已保存在本机")).font(.subheadline).foregroundStyle(Palette.secondary)
                        Divider().padding(.vertical, 10)
                        Text(L10n.tr("只安排两件想做的事，给临时起意留点余地。")).font(.body).lineSpacing(5)
                        ForEach($draft.sections) { $section in
                            Text(section.title).font(.title3.weight(.semibold)).padding(.top, 10).padding(.bottom, 8)
                            ForEach($section.items) { $item in
                                Button { draft.toggleItem(sectionID: section.id, itemID: item.id) } label: {
                                    HStack(alignment: .top, spacing: 12) {
                                        Image(systemName: item.isDone ? "checkmark.square.fill" : "square").font(.system(size: 22, weight: .light)).frame(width: 25)
                                        Text(item.text).strikethrough(item.isDone).foregroundStyle(item.isDone ? Palette.secondary : Palette.ink).multilineTextAlignment(.leading).lineSpacing(3)
                                        Spacer(minLength: 0)
                                    }.frame(minHeight: 44, alignment: .top).contentShape(Rectangle())
                                }.buttonStyle(.plain).accessibilityLabel(item.text).accessibilityValue(item.isDone ? L10n.tr("已完成") : L10n.tr("未完成"))
                            }
                        }
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 22).padding(.vertical, 14)
            }.scrollDismissesKeyboard(.interactively).accessibilityIdentifier("draft-content")
            if !keyboardVisible {
                HStack(spacing: 24) {
                    Button { UIPasteboard.general.string = draft.markdown; copied = true } label: { Label(copied ? L10n.tr("已复制") : L10n.tr("复制"), systemImage: copied ? "checkmark" : "doc.on.doc").frame(minHeight: 44) }.accessibilityIdentifier("copy-document")
                    Divider().frame(height: 18)
                    Button { share(draft.markdown) } label: { Label(L10n.tr("分享"), systemImage: "square.and.arrow.up").frame(minHeight: 44) }.accessibilityIdentifier("share-document")
                    Spacer(minLength: 0)
                    IconButton(symbol: "clock.arrow.circlepath", label: L10n.tr("文稿历史版本"), id: "draft-history") { showingHistory = true }
                }.font(.subheadline).padding(.horizontal, 24).overlay(alignment: .top) { Divider().padding(.horizontal, 22) }
            }
        }
        .background(Palette.surface, in: UnevenRoundedRectangle(topLeadingRadius: 28, topTrailingRadius: 28))
        .overlay { UnevenRoundedRectangle(topLeadingRadius: 28, topTrailingRadius: 28).stroke(Palette.line, lineWidth: 0.6).allowsHitTesting(false) }
        .onChange(of: draft.markdown) { _, _ in copied = false }
        .sheet(isPresented: $showingHistory) { DraftHistoryView(draft: $draft) }
        .sheet(isPresented: $editing) {
            TextEditingSheet(title: L10n.tr("编辑文稿"), text: $editText, saveLabel: L10n.tr("保存")) {
                draft.replaceContent(editText)
                editing = false
            }
        }
    }
}

struct TextEditingSheet: View {
    let title: String
    @Binding var text: String
    var saveLabel = L10n.tr("保存")
    var save: () -> Void
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            TextEditor(text: $text).font(.body).padding(16).scrollContentBackground(.hidden).background(Palette.canvas).accessibilityIdentifier("text-editor")
                .navigationTitle(title).navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { dismiss() } }
                    ToolbarItem(placement: .confirmationAction) { Button(saveLabel, action: save).bold().disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).accessibilityIdentifier("save-edit") }
                }
        }.interactiveDismissDisabled()
    }
}

struct MarkdownContent: View {
    let text: String
    var streaming = false
    @State private var renderCache = MarkdownRenderCache()
    var toggleChecklist: ((Int) -> Void)? = nil
    var codeBackground: Color = Palette.canvas
    var codeBorder: Color = .clear
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(Array(renderCache.blocks(for: text, streaming: streaming).enumerated()), id: \.offset) { index, block in
                switch block {
                case .heading(let level, let value):
                    Text(value).font(level == 1 ? .title2.bold() : level == 2 ? .title3.bold() : .headline).padding(.top, 6).textSelection(.enabled)
                case .paragraph(let value):
                    Text(inline(value)).lineSpacing(5).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                case .list(let marker, let value):
                    if (marker == "☑" || marker == "☐"), let toggleChecklist {
                        Button {
                            let preceding = renderCache.blocks(for: text, streaming: streaming).prefix(index).filter { if case .list(let marker, _) = $0 { return marker == "☑" || marker == "☐" }; return false }.count
                            toggleChecklist(preceding)
                        } label: { HStack(alignment: .top, spacing: 10) { Image(systemName: marker == "☑" ? "checkmark.square.fill" : "square"); Text(inline(value)).strikethrough(marker == "☑").frame(maxWidth: .infinity, alignment: .leading) }.frame(minHeight: 44).contentShape(Rectangle()) }.buttonStyle(.plain).accessibilityLabel(value).accessibilityValue(marker == "☑" ? L10n.tr("已完成") : L10n.tr("未完成"))
                    } else {
                        HStack(alignment: .top, spacing: 10) { Group { if marker == "☑" || marker == "☐" { Image(systemName: marker == "☑" ? "checkmark.square" : "square") } else { Text(marker) } }.foregroundStyle(Palette.secondary); Text(inline(value)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }.lineSpacing(4)
                    }
                case .quote(let value):
                    HStack(spacing: 12) { Rectangle().fill(Palette.line).frame(width: 3); Text(inline(value)).foregroundStyle(Palette.secondary).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }.fixedSize(horizontal: false, vertical: true)
                case .code(let language, let value):
                    VStack(alignment: .leading, spacing: 0) {
                        HStack { Text(language.isEmpty ? L10n.tr("代码") : language).font(.caption); Spacer(); Button { UIPasteboard.general.string = value } label: { Label(L10n.tr("复制"), systemImage: "doc.on.doc").font(.caption).frame(minHeight: 44) }.accessibilityIdentifier("copy-code") }.padding(.horizontal, 14)
                        Divider()
                        ScrollView(.horizontal) { HighlightedCode(code: value, language: language).padding(14) }
                            .excludesSidebarGesture().accessibilityIdentifier("markdown-code-scroll")
                    }.background(codeBackground, in: RoundedRectangle(cornerRadius: 12))
                        .overlay { RoundedRectangle(cornerRadius: 12).stroke(codeBorder, lineWidth: 1) }
                case .table(let rows):
                    VStack(alignment: .leading, spacing: 6) {
                        ScrollView(.horizontal) {
                            Grid(alignment: .leading, horizontalSpacing: 0, verticalSpacing: 0) {
                                ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                                    GridRow { ForEach(Array(row.enumerated()), id: \.offset) { _, cell in Text(inline(cell)).font(index == 0 ? .subheadline.bold() : .subheadline).frame(minWidth: 90, maxWidth: 220, alignment: .leading).padding(12).background(index == 0 ? Palette.muted : Palette.canvas).border(Palette.line, width: 0.5) } }
                                }
                            }.textSelection(.enabled)
                        }.scrollIndicators(.visible)
                            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                            .overlay { RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Palette.line, lineWidth: 1).allowsHitTesting(false) }
                            .excludesSidebarGesture().accessibilityIdentifier("markdown-table-scroll")
                        // Wide tables continue off screen; say so instead of silently clipping columns.
                        if (rows.map(\.count).max() ?? 0) >= 4 {
                            Label(L10n.tr("左右滑动查看完整表格"), systemImage: "arrow.left.and.right").font(.caption2).foregroundStyle(Palette.secondary).accessibilityHidden(true)
                        }
                    }
                case .divider: Divider().padding(.vertical, 4)
                }
            }
        }
    }
    private func inline(_ value: String) -> AttributedString {
        renderCache.inline(value)
    }
}

struct IconButton: View {
    let symbol: String
    let label: String
    var id = ""
    var action: () -> Void
    var body: some View {
        Button(action: action) { Image(systemName: symbol).font(.system(size: 20)).frame(width: 44, height: 44).contentShape(Circle()) }
            .buttonStyle(.plain).accessibilityLabel(label).accessibilityIdentifier(id)
    }
}
