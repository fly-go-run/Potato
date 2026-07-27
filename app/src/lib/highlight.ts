import type {
  LanguageInput,
  LanguageRegistration,
  ThemedToken,
} from "shiki/core";

type LanguageLoader = () => Promise<{ default: LanguageInput }>;

const languageLoaders: Record<string, LanguageLoader> = {
  typescript: () => import("@shikijs/langs/typescript"),
  javascript: () => import("@shikijs/langs/javascript"),
  tsx: () => import("@shikijs/langs/tsx"),
  json: () => import("@shikijs/langs/json"),
  bash: () => import("@shikijs/langs/bash"),
  python: () => import("@shikijs/langs/python"),
  html: () => import("@shikijs/langs/html"),
  css: () => import("@shikijs/langs/css"),
  markdown: () => import("@shikijs/langs/markdown"),
  yaml: () => import("@shikijs/langs/yaml"),
  sql: () => import("@shikijs/langs/sql"),
  go: () => import("@shikijs/langs/go"),
  rust: () => import("@shikijs/langs/rust"),
  java: () => import("@shikijs/langs/java"),
  c: () => import("@shikijs/langs/c"),
  cpp: async () => ({ default: [cppLanguage] }),
  diff: () => import("@shikijs/langs/diff"),
};

const aliases: Record<string, string> = {
  ts: "typescript",
  js: "javascript",
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  py: "python",
  htm: "html",
  md: "markdown",
  yml: "yaml",
  rs: "rust",
  "c++": "cpp",
};

const cppLanguage: LanguageRegistration = {
  name: "cpp",
  aliases: ["c++"],
  scopeName: "source.cpp",
  patterns: [
    { include: "#comments" },
    { include: "#strings" },
    {
      match:
        "\\b(?:alignas|alignof|asm|auto|break|case|catch|class|concept|consteval|constexpr|constinit|continue|co_await|co_return|co_yield|default|delete|do|else|enum|explicit|export|extern|for|friend|goto|if|inline|namespace|new|noexcept|operator|private|protected|public|requires|return|sizeof|static|struct|switch|template|this|throw|try|typedef|typename|union|using|virtual|while)\\b",
      name: "keyword.control.cpp",
    },
    {
      match:
        "\\b(?:bool|char|char8_t|char16_t|char32_t|double|float|int|long|short|signed|unsigned|void|wchar_t)\\b",
      name: "storage.type.cpp",
    },
    {
      match: "\\b(?:false|nullptr|true)\\b",
      name: "constant.language.cpp",
    },
    {
      match: "\\b(?:0[xX][0-9a-fA-F]+|0[bB][01]+|\\d+(?:\\.\\d+)?)\\b",
      name: "constant.numeric.cpp",
    },
    {
      match: "^\\s*#\\s*[a-zA-Z_][a-zA-Z0-9_]*",
      name: "keyword.control.directive.cpp",
    },
    {
      match: "\\b[a-zA-Z_][a-zA-Z0-9_]*(?=\\s*\\()",
      name: "entity.name.function.cpp",
    },
  ],
  repository: {
    comments: {
      patterns: [
        {
          begin: "/\\*",
          end: "\\*/",
          name: "comment.block.cpp",
        },
        {
          begin: "//",
          end: "$",
          name: "comment.line.double-slash.cpp",
        },
      ],
    },
    strings: {
      patterns: [
        {
          begin: '"',
          end: '"',
          name: "string.quoted.double.cpp",
          patterns: [
            {
              match: "\\\\.",
              name: "constant.character.escape.cpp",
            },
          ],
        },
        {
          begin: "'",
          end: "'",
          name: "string.quoted.single.cpp",
          patterns: [
            {
              match: "\\\\.",
              name: "constant.character.escape.cpp",
            },
          ],
        },
      ],
    },
  },
};

let highlighterPromise:
  | Promise<
      Awaited<
        ReturnType<
          typeof import("shiki/core")["createHighlighterCore"]
        >
      >
    >
  | null = null;
const languagePromises = new Map<string, Promise<void>>();

export function isSupportedLanguage(language?: string): boolean {
  if (!language) return false;
  const normalized = aliases[language.toLowerCase()] ?? language.toLowerCase();
  return normalized in languageLoaders;
}

export async function highlightCode(
  code: string,
  language?: string,
): Promise<ThemedToken[][] | null> {
  if (!isSupportedLanguage(language)) return null;
  const normalized =
    aliases[language!.toLowerCase()] ?? language!.toLowerCase();
  const highlighter = await getHighlighter();
  await loadLanguage(highlighter, normalized);
  return highlighter.codeToTokens(code, {
    lang: normalized,
    theme: "github-light",
    includeExplanation: "scopeName",
  }).tokens;
}

async function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = Promise.all([
      import("shiki/core"),
      import("shiki/engine/javascript"),
      import("@shikijs/themes/github-light"),
    ]).then(([{ createHighlighterCore }, { createJavaScriptRegexEngine }, theme]) =>
      createHighlighterCore({
        themes: [theme.default],
        langs: [],
        engine: createJavaScriptRegexEngine(),
      }),
    );
  }
  return highlighterPromise;
}

async function loadLanguage(
  highlighter: Awaited<ReturnType<typeof getHighlighter>>,
  language: string,
) {
  if (highlighter.getLoadedLanguages().includes(language)) return;
  let pending = languagePromises.get(language);
  if (!pending) {
    pending = languageLoaders[language]!()
      .then((module) => highlighter.loadLanguage(module.default))
      .then(() => undefined);
    languagePromises.set(language, pending);
  }
  await pending;
}
