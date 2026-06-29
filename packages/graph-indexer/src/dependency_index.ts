export type ParsedDependency = {
  sourcePath: string;
  specifier: string;
};

export type OxcTooling = {
  parser: "oxc-parser";
  transform: "oxc-transform";
};

export function getOxcTooling(): OxcTooling {
  return {
    parser: "oxc-parser",
    transform: "oxc-transform",
  };
}
