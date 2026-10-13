import type { languages } from "monaco-editor";

export const TOML_LANGUAGE = "toml";

export const tomlConfiguration: languages.LanguageConfiguration = {
  comments: { lineComment: "#" },
  brackets: [
    ["[", "]"],
    ["{", "}"],
  ],
  autoClosingPairs: [
    { open: "[", close: "]" },
    { open: "{", close: "}" },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
};

// Monarch has no `^` anchor, so values live in their own state that ends with the line; a bracket in the
// root state is then always a table header, including quoted ones like ["theme.ogg"].
export const tomlMonarch: languages.IMonarchLanguage = {
  tokenizer: {
    root: [
      [/#.*$/, "comment"],
      [/\[\[[^\]]*\]\]|\[[^\]]*\]/, "type.label"],
      [/[\w-]+|"[^"]*"|'[^']*'/, "attribute"],
      [/\./, "operator"],
      [/=/, "operator", "@value"],
    ],
    value: [[/$/, "", "@pop"], { include: "@values" }],
    array: [[/\]/, "operator", "@pop"], { include: "@values" }],
    inline: [[/\}/, "operator", "@pop"], [/[\w-]+(?=\s*=)/, "attribute"], { include: "@values" }],
    values: [
      [/#.*$/, "comment"],
      [/"""/, "string", "@multiBasic"],
      [/'''/, "string", "@multiLiteral"],
      [/"/, "string", "@basic"],
      [/'[^']*'/, "string"],
      [/\b(true|false)\b/, "constant"],
      [/\d{4}-\d{2}-\d{2}([Tt ][\d:.]+)?([Zz]|[+-]\d{2}:\d{2})?/, "number"],
      [/[+-]?(inf|nan)\b/, "number"],
      [/[+-]?(0x[\da-fA-F_]+|0o[0-7_]+|0b[01_]+)/, "number"],
      [/[+-]?\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?/, "number"],
      [/\[/, "operator", "@array"],
      [/\{/, "operator", "@inline"],
      [/[,=.]/, "operator"],
    ],
    basic: [
      [/\\./, "string.escape"],
      [/"/, "string", "@pop"],
      [/[^"\\]+/, "string"],
    ],
    multiBasic: [
      [/\\./, "string.escape"],
      [/"""/, "string", "@pop"],
      [/[^"\\]+|"/, "string"],
    ],
    multiLiteral: [
      [/'''/, "string", "@pop"],
      [/[^']+|'/, "string"],
    ],
  },
};
