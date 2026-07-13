import { evaluate } from "@mdx-js/mdx";
import type { ComponentPropsWithoutRef, ComponentType } from "react";
import { useEffect, useState } from "react";
import { Fragment, jsx, jsxs } from "react/jsx-runtime";
import { createHighlighter } from "shiki";

import {
  SHIKI_DARK_THEME,
  SHIKI_LANGUAGE_IDS,
  SHIKI_LIGHT_THEME,
} from "@/shared/lib/editor/monaco-shiki";

const highlighterPromise = createHighlighter({
  themes: [SHIKI_LIGHT_THEME, SHIKI_DARK_THEME],
  langs: [...SHIKI_LANGUAGE_IDS],
});

const languageAliases = new Map([
  ["ts", "typescript"],
  ["js", "javascript"],
  ["py", "python"],
  ["sh", "shell"],
  ["yml", "yaml"],
  ["md", "markdown"],
]);

export function MdxDescription({ source }: { source: string }) {
  const [content, setContent] = useState<ComponentType | null>(null);
  const [renderError, setRenderError] = useState("");

  useEffect(() => {
    let disposed = false;

    async function compileMarkdown() {
      try {
        const module = await evaluate(source, { Fragment, format: "md", jsx, jsxs });
        if (!disposed) {
          setContent(() => module.default);
          setRenderError("");
        }
      } catch (error) {
        if (!disposed) {
          setContent(null);
          setRenderError(error instanceof Error ? error.message : String(error));
        }
      }
    }

    void compileMarkdown();
    return () => {
      disposed = true;
    };
  }, [source]);

  if (renderError) {
    return <pre className="whitespace-pre-wrap">{source}</pre>;
  }

  const Content = content as ComponentType<{
    components?: { code: typeof HighlightedCode };
  }> | null;
  return Content ? <Content components={{ code: HighlightedCode }} /> : null;
}

function HighlightedCode({ className, children, ...props }: ComponentPropsWithoutRef<"code">) {
  const language = className?.match(/language-(\S+)/)?.[1];
  const source = codeChildrenToString(children).replace(/\n$/, "");
  const [highlightedHtml, setHighlightedHtml] = useState<string | null>(null);

  useEffect(() => {
    if (!language) return;

    let disposed = false;
    const resolvedLanguage = languageAliases.get(language) ?? language;

    void highlighterPromise.then((highlighter) => {
      if (disposed) return;

      try {
        const html = highlighter.codeToHtml(source, {
          lang: resolvedLanguage,
          themes: { dark: SHIKI_DARK_THEME, light: SHIKI_LIGHT_THEME },
        });
        setHighlightedHtml(html.replace(/^<pre[^>]*><code>/, "").replace(/<\/code><\/pre>$/, ""));
      } catch {
        setHighlightedHtml(null);
      }
    });

    return () => {
      disposed = true;
    };
  }, [language, source]);

  if (!language || !highlightedHtml) {
    return (
      <code className={className} {...props}>
        {children}
      </code>
    );
  }

  return (
    <code
      className={className ? `${className} shiki` : "shiki"}
      {...props}
      dangerouslySetInnerHTML={{ __html: highlightedHtml }}
    />
  );
}

function codeChildrenToString(children: ComponentPropsWithoutRef<"code">["children"]): string {
  if (typeof children === "string" || typeof children === "number") {
    return String(children);
  }

  if (Array.isArray(children)) {
    return children.map(codeChildrenToString).join("");
  }

  return "";
}
