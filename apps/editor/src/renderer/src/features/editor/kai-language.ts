import type { languages } from "monaco-editor";

export const KAI_LANGUAGE = "kairos";

export const KAI_KEYWORDS = [
  "label",
  "jump",
  "call",
  "return",
  "pass",
  "menu",
  "if",
  "elif",
  "else",
  "character",
  "define",
  "default",
  "scene",
  "show",
  "hide",
  "with",
  "at",
  "play",
  "queue",
  "stop",
  "loop",
  "fadein",
  "fadeout",
  "pause",
  "transform",
  "and",
  "or",
  "not",
];

export const kaiConfiguration: languages.LanguageConfiguration = {
  comments: { lineComment: "#" },
  brackets: [["(", ")"]],
  autoClosingPairs: [
    { open: '"', close: '"', notIn: ["string"] },
    { open: "(", close: ")" },
    { open: "[", close: "]", notIn: [] },
  ],
  indentationRules: {
    increaseIndentPattern: /:\s*(#.*)?$/,
    decreaseIndentPattern: /^\s*(elif|else)\b/,
  },
  onEnterRules: [],
  wordPattern: /[A-Za-z_]\w*|-?\d+(\.\d+)?/,
};

export const kaiMonarch: languages.IMonarchLanguage = {
  defaultToken: "",
  keywords: KAI_KEYWORDS,
  constants: ["True", "False", "None"],
  tokenizer: {
    root: [
      [/#.*$/, "comment"],
      [/^(\s*)(label|transform)(\s+)([A-Za-z_]\w*)/, ["", "keyword", "", "type.label"]],
      [/\b(jump|call)(\s+)([A-Za-z_]\w*)/, ["keyword.flow", "", "type.label"]],
      [/^(\s*)(character)(\s+)([A-Za-z_]\w*)/, ["", "keyword", "", "variable.speaker"]],
      [/^(\s*)(?!(?:menu|if|elif|else)\b)([A-Za-z_]\w*)(\s+)(?=")/, ["", "variable.speaker", ""]],
      [/\$/, "keyword.operator"],
      [/[A-Za-z_]\w*(?==)/, "attribute"],
      [/[A-Za-z_]\w*/, { cases: { "@keywords": "keyword", "@constants": "constant", "@default": "identifier" } }],
      [/\d+(\.\d+)?/, "number"],
      [/"/, "string", "@string"],
      [/[=+\-*/<>!%]=?|:/, "operator"],
    ],
    string: [
      [/\[[^\]"]*\]/, "string.interpolation"],
      [/[^\\"[]+/, "string"],
      [/\\./, "string.escape"],
      [/\[/, "string"],
      [/"/, "string", "@pop"],
    ],
  },
};
