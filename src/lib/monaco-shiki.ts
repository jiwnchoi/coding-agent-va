import { loader } from "@monaco-editor/react";
import { shikiToMonaco } from "@shikijs/monaco";
import type { languages } from "monaco-editor";
import { createHighlighter } from "shiki";

export const SHIKI_LIGHT_THEME = "vitesse-light";
export const SHIKI_DARK_THEME = "vitesse-dark";

const SHIKI_LANGUAGE_IDS = [
  "typescript",
  "javascript",
  "tsx",
  "jsx",
  "json",
  "html",
  "css",
  "scss",
  "markdown",
  "yaml",
  "toml",
  "rust",
  "go",
  "java",
  "python",
  "shell",
  "bash",
  "dockerfile",
  "sql",
  "swift",
  "kotlin",
  "c",
  "cpp",
  "csharp",
  "php",
  "ruby",
  "xml",
  "plaintext",
] as const;

const FILE_NAME_LANGUAGE_MAP = new Map<string, string>([
  ["dockerfile", "dockerfile"],
  [".gitignore", "plaintext"],
  [".env", "shell"],
]);

const FILE_EXTENSION_LANGUAGE_MAP = new Map<string, string>([
  ["ts", "typescript"],
  ["tsx", "tsx"],
  ["js", "javascript"],
  ["jsx", "jsx"],
  ["mjs", "javascript"],
  ["cjs", "javascript"],
  ["mts", "typescript"],
  ["cts", "typescript"],
  ["json", "json"],
  ["jsonl", "json"],
  ["html", "html"],
  ["css", "css"],
  ["scss", "scss"],
  ["md", "markdown"],
  ["mdx", "markdown"],
  ["yaml", "yaml"],
  ["yml", "yaml"],
  ["toml", "toml"],
  ["rs", "rust"],
  ["go", "go"],
  ["java", "java"],
  ["py", "python"],
  ["sh", "shell"],
  ["bash", "bash"],
  ["zsh", "shell"],
  ["fish", "shell"],
  ["sql", "sql"],
  ["swift", "swift"],
  ["kt", "kotlin"],
  ["kts", "kotlin"],
  ["c", "c"],
  ["h", "c"],
  ["cpp", "cpp"],
  ["cc", "cpp"],
  ["cxx", "cpp"],
  ["hpp", "cpp"],
  ["cs", "csharp"],
  ["php", "php"],
  ["rb", "ruby"],
  ["xml", "xml"],
  ["svg", "xml"],
]);

let shikiMonacoInitializationPromise: Promise<void> | null = null;

export function resolveMonacoLanguage(filePath: string) {
  const normalizedFilePath = filePath.replace(/\\/g, "/");
  const fileName = normalizedFilePath.split("/").pop()?.toLowerCase() ?? "";
  const directLanguage = FILE_NAME_LANGUAGE_MAP.get(fileName);

  if (directLanguage) {
    return directLanguage;
  }

  const extension = fileName.split(".").pop();

  if (!extension || !fileName.includes(".")) {
    return "plaintext";
  }

  return FILE_EXTENSION_LANGUAGE_MAP.get(extension) ?? "plaintext";
}

export async function ensureShikiMonaco() {
  if (!shikiMonacoInitializationPromise) {
    shikiMonacoInitializationPromise = (async () => {
      const monaco = await loader.init();
      const highlighter = await createHighlighter({
        themes: [SHIKI_LIGHT_THEME, SHIKI_DARK_THEME],
        langs: [...SHIKI_LANGUAGE_IDS],
      });

      for (const languageId of SHIKI_LANGUAGE_IDS) {
        const isRegistered = monaco.languages
          .getLanguages()
          .some((language: languages.ILanguageExtensionPoint) => language.id === languageId);

        if (!isRegistered) {
          monaco.languages.register({ id: languageId });
        }
      }

      shikiToMonaco(highlighter, monaco);
    })();
  }

  return shikiMonacoInitializationPromise;
}
