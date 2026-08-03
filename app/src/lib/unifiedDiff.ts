/**
 * git unified diff 文本解析。只依赖字符串,不做任何请求;
 * ChangeDiffPanel 用它把 /api/workspace/git/diff 的输出渲染成带行号的视图。
 */

export interface UnifiedDiffLine {
  kind: "add" | "remove" | "context";
  /** 去掉行首 +/-/空格 后的内容。 */
  text: string;
  /** 旧文件行号;新增行为 null。 */
  oldLine: number | null;
  /** 新文件行号;删除行为 null。 */
  newLine: number | null;
}

export interface UnifiedDiffHunk {
  /** @@ 之后的上下文说明(函数名等),可能为空串。 */
  section: string;
  oldStart: number;
  oldCount: number;
  newStart: number;
  newCount: number;
  lines: UnifiedDiffLine[];
}

export interface UnifiedFileDiff {
  /** 旧/新路径,已去掉 a// b/ 前缀;/dev/null 记为 ""。 */
  oldPath: string;
  newPath: string;
  isNew: boolean;
  isDeleted: boolean;
  isRename: boolean;
  isBinary: boolean;
  hunks: UnifiedDiffHunk[];
  additions: number;
  deletions: number;
}

const HUNK_HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@ ?(.*)$/;

export function parseUnifiedDiff(text: string): UnifiedFileDiff[] {
  const lines = text.split("\n");
  const files: UnifiedFileDiff[] = [];
  let file: UnifiedFileDiff | null = null;
  let hunk: UnifiedDiffHunk | null = null;
  let oldLine = 0;
  let newLine = 0;

  const closeHunk = () => {
    hunk = null;
  };
  const closeFile = () => {
    closeHunk();
    if (file) files.push(file);
    file = null;
  };

  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index]!;
    // git 在 CRLF 文件里会原样带回 \r,统一剥掉末尾一个。
    const line = rawLine.endsWith("\r") ? rawLine.slice(0, -1) : rawLine;

    if (line.startsWith("diff --git ")) {
      closeFile();
      // 从头行尽力回填路径:二进制 diff 没有 ---/+++ 行,只有这里有。
      // 含空格的未引号路径存在歧义,后续 ---/+++ 行(若有)会覆盖。
      const headerPaths =
        /^diff --git (?:"a\/(.+?)"|a\/(\S+)) (?:"b\/(.+?)"|b\/(\S+))$/.exec(
          line,
        );
      file = {
        oldPath: headerPaths?.[1] ?? headerPaths?.[2] ?? "",
        newPath: headerPaths?.[3] ?? headerPaths?.[4] ?? "",
        isNew: false,
        isDeleted: false,
        isRename: false,
        isBinary: false,
        hunks: [],
        additions: 0,
        deletions: 0,
      };
      continue;
    }
    if (!file) continue;

    if (!hunk) {
      // 文件头区
      if (line.startsWith("new file mode")) {
        file.isNew = true;
        continue;
      }
      if (line.startsWith("deleted file mode")) {
        file.isDeleted = true;
        continue;
      }
      if (line.startsWith("rename from ")) {
        file.isRename = true;
        file.oldPath = line.slice("rename from ".length);
        continue;
      }
      if (line.startsWith("rename to ")) {
        file.isRename = true;
        file.newPath = line.slice("rename to ".length);
        continue;
      }
      if (line.startsWith("Binary files ") || line === "GIT binary patch") {
        file.isBinary = true;
        continue;
      }
      if (line.startsWith("--- ")) {
        file.oldPath = stripDiffPath(line.slice(4));
        continue;
      }
      if (line.startsWith("+++ ")) {
        file.newPath = stripDiffPath(line.slice(4));
        continue;
      }
    }

    const header = HUNK_HEADER.exec(line);
    if (header) {
      hunk = {
        section: header[5] ?? "",
        oldStart: Number(header[1]),
        oldCount: header[2] === undefined ? 1 : Number(header[2]),
        newStart: Number(header[3]),
        newCount: header[4] === undefined ? 1 : Number(header[4]),
        lines: [],
      };
      file.hunks.push(hunk);
      oldLine = hunk.oldStart;
      newLine = hunk.newStart;
      continue;
    }
    if (!hunk) continue;

    // hunk 体
    if (line.startsWith("+")) {
      hunk.lines.push({
        kind: "add",
        text: line.slice(1),
        oldLine: null,
        newLine: newLine,
      });
      newLine += 1;
      file.additions += 1;
    } else if (line.startsWith("-")) {
      hunk.lines.push({
        kind: "remove",
        text: line.slice(1),
        oldLine: oldLine,
        newLine: null,
      });
      oldLine += 1;
      file.deletions += 1;
    } else if (line.startsWith(" ") || line === "") {
      // 空串只可能是 diff 末尾的收尾换行;真正的空上下文行带一个前导空格。
      if (line === "" && index === lines.length - 1) continue;
      hunk.lines.push({
        kind: "context",
        text: line.slice(1),
        oldLine: oldLine,
        newLine: newLine,
      });
      oldLine += 1;
      newLine += 1;
    } else if (line.startsWith("\\")) {
      // "\ No newline at end of file" 标记行,不参与行号推进。
      continue;
    } else {
      // hunk 体外的未知行(下一段文件头等),关闭当前 hunk 交回头区处理。
      closeHunk();
    }
  }
  closeFile();
  return files;
}

function stripDiffPath(value: string): string {
  // 形如 "a/src/x.ts"、"b/src/x.ts"、"/dev/null";引号包裹的转义路径原样保留内容。
  // git 对含空格路径会在行尾加 \t 终止符,先剥掉。
  const trimmed = value.replace(/\t$/, "");
  const unquoted =
    trimmed.startsWith('"') && trimmed.endsWith('"')
      ? trimmed.slice(1, -1)
      : trimmed;
  if (unquoted === "/dev/null") return "";
  return unquoted.replace(/^[ab]\//, "");
}

/**
 * 把工具调用里的绝对路径映射成 git status 给出的仓库相对路径。
 * 后端不暴露仓库根,只能做后缀匹配;取匹配段数最多(最specific)的候选,
 * 避免 "b/x.ts" 同时命中 "x.ts" 与 "a/b/x.ts" 时选错。
 */
export function matchRepoRelativePath(
  absolutePath: string,
  candidates: string[],
): string | null {
  const normalized = absolutePath.replaceAll("\\", "/");
  let best: string | null = null;
  let bestSegments = 0;
  for (const candidate of candidates) {
    const relative = candidate.replaceAll("\\", "/").replace(/^\.\//, "");
    if (!relative) continue;
    const matched =
      normalized === relative || normalized.endsWith(`/${relative}`);
    if (!matched) continue;
    const segments = relative.split("/").length;
    if (segments > bestSegments) {
      best = candidate;
      bestSegments = segments;
    }
  }
  return best;
}
