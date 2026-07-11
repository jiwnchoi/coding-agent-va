import cIcon from "material-icon-theme/icons/c.svg";
import bashIcon from "material-icon-theme/icons/console.svg";
import cppIcon from "material-icon-theme/icons/cpp.svg";
import csharpIcon from "material-icon-theme/icons/csharp.svg";
import cssIcon from "material-icon-theme/icons/css.svg";
import defaultFileIcon from "material-icon-theme/icons/file.svg";
import goIcon from "material-icon-theme/icons/go.svg";
import htmlIcon from "material-icon-theme/icons/html.svg";
import javaIcon from "material-icon-theme/icons/java.svg";
import javascriptIcon from "material-icon-theme/icons/javascript.svg";
import jsonIcon from "material-icon-theme/icons/json.svg";
import kotlinIcon from "material-icon-theme/icons/kotlin.svg";
import phpIcon from "material-icon-theme/icons/php.svg";
import pythonIcon from "material-icon-theme/icons/python.svg";
import reactIcon from "material-icon-theme/icons/react.svg";
import rubyIcon from "material-icon-theme/icons/ruby.svg";
import rustIcon from "material-icon-theme/icons/rust.svg";
import swiftIcon from "material-icon-theme/icons/swift.svg";
import tomlIcon from "material-icon-theme/icons/toml.svg";
import typescriptIcon from "material-icon-theme/icons/typescript.svg";
import yamlIcon from "material-icon-theme/icons/yaml.svg";

const iconByLanguage: Record<string, string> = {
  bash: bashIcon,
  c: cIcon,
  cpp: cppIcon,
  csharp: csharpIcon,
  css: cssIcon,
  go: goIcon,
  html: htmlIcon,
  java: javaIcon,
  javascript: javascriptIcon,
  json: jsonIcon,
  jsx: reactIcon,
  kotlin: kotlinIcon,
  php: phpIcon,
  python: pythonIcon,
  ruby: rubyIcon,
  rust: rustIcon,
  swift: swiftIcon,
  toml: tomlIcon,
  tsx: reactIcon,
  typescript: typescriptIcon,
  yaml: yamlIcon,
};

export function fileIconForLanguage(language: string) {
  return iconByLanguage[language] ?? defaultFileIcon;
}
