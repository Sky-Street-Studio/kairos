import { themeVariables, type ResolvedAppearance, type ThemeConfig } from "@kairos/theme";
import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";

import { appStore } from "@/store";

import { KAI_LANGUAGE, kaiConfiguration, kaiMonarch } from "./kai-language";

export { monaco };

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };

export const modelUri = (path: string) => monaco.Uri.from({ scheme: "kairos", path: `/${path}` });
export const pathOfUri = (uri: monaco.Uri) => uri.path.replace(/^\//, "");

export function ensureModel(path: string, source: string): monaco.editor.ITextModel {
  const uri = modelUri(path);
  return monaco.editor.getModel(uri) ?? monaco.editor.createModel(source, KAI_LANGUAGE, uri);
}

function labelAt(model: monaco.editor.ITextModel, position: monaco.Position) {
  const word = model.getWordAtPosition(position);
  if (!word) return null;
  const before = model.getLineContent(position.lineNumber).slice(0, word.startColumn - 1);
  if (!/\b(jump|call)\s+$/.test(before)) return null;
  const node = appStore()
    .getState()
    .analysis.graph.nodes.find((n) => n.label === word.word);
  return node ? { node, word } : null;
}

const SYNTAX = {
  dark: {
    comment: "6f7389",
    label: "7cc4ff",
    speaker: "f2c46b",
    string: "a9d99a",
    interpolation: "e7a6ff",
    number: "f5a37c",
    attribute: "9ab0d6",
  },
  light: {
    comment: "7a7d8e",
    label: "1660b0",
    speaker: "985600",
    string: "2d7428",
    interpolation: "8a3cc0",
    number: "b34b1a",
    attribute: "4a5a7a",
  },
} as const;

export function defineTheme(theme: ThemeConfig, appearance: ResolvedAppearance): string {
  const v = themeVariables(theme, appearance);
  const c = SYNTAX[appearance];
  const hex = (s: string) => s.replace("#", "");
  const name = `kairos-${appearance}`;
  monaco.editor.defineTheme(name, {
    base: appearance === "dark" ? "vs-dark" : "vs",
    inherit: true,
    rules: [
      { token: "comment", foreground: c.comment, fontStyle: "italic" },
      { token: "keyword", foreground: hex(v.primary) },
      { token: "keyword.flow", foreground: hex(v.primary), fontStyle: "bold" },
      { token: "keyword.operator", foreground: hex(v.primary) },
      { token: "type.label", foreground: c.label },
      { token: "variable.speaker", foreground: c.speaker },
      { token: "string", foreground: c.string },
      { token: "string.interpolation", foreground: c.interpolation },
      { token: "string.escape", foreground: c.interpolation },
      { token: "number", foreground: c.number },
      { token: "constant", foreground: c.number },
      { token: "attribute", foreground: c.attribute },
      { token: "operator", foreground: hex(v["muted-foreground"]) },
    ],
    colors: {
      "editor.background": "#00000000",
      "editor.foreground": v.foreground,
      "editorGutter.background": "#00000000",
      "minimap.background": "#00000000",
      "editor.lineHighlightBackground": `${v.foreground}0a`,
      "editor.lineHighlightBorder": "#00000000",
      "editor.selectionBackground": v.selection,
      "editor.inactiveSelectionBackground": `${v.primary}22`,
      "editorCursor.foreground": v.primary,
      "editorLineNumber.foreground": `${v["muted-foreground"]}99`,
      "editorLineNumber.activeForeground": v.foreground,
      "editorIndentGuide.background1": `${v.foreground}12`,
      "editorIndentGuide.activeBackground1": `${v.foreground}30`,
      "editorWidget.background": v.popover,
      "editorWidget.border": v.border,
      "editorHoverWidget.background": v.popover,
      "editorHoverWidget.border": v.border,
      "editorSuggestWidget.background": v.popover,
      "editorSuggestWidget.border": v.border,
      "editorSuggestWidget.selectedBackground": v.accent,
      "editorError.foreground": v.destructive,
      "editorWarning.foreground": v.warning,
      "editorOverviewRuler.border": "#00000000",
      "scrollbar.shadow": "#00000000",
      "scrollbarSlider.background": `${v.foreground}22`,
      "scrollbarSlider.hoverBackground": `${v.foreground}38`,
      "menu.background": v.popover,
      "menu.foreground": v.foreground,
      "menu.selectionBackground": v.accent,
      "menu.selectionForeground": v.foreground,
      "menu.border": v.border,
      focusBorder: `${v.primary}88`,
      "list.hoverBackground": v.accent,
      "list.activeSelectionBackground": v.accent,
      "input.background": v.background,
      "input.border": v.border,
    },
  });
  return name;
}

let registered = false;

export function registerKairosLanguage(): void {
  if (registered) return;
  registered = true;
  monaco.languages.register({ id: KAI_LANGUAGE, extensions: [".kai"], aliases: ["Kairos"] });
  monaco.languages.setLanguageConfiguration(KAI_LANGUAGE, kaiConfiguration);
  monaco.languages.setMonarchTokensProvider(KAI_LANGUAGE, kaiMonarch);

  monaco.languages.registerDefinitionProvider(KAI_LANGUAGE, {
    provideDefinition(model, position) {
      const hit = labelAt(model, position);
      if (!hit) return null;
      const target = ensureModel(hit.node.path, appStore().getState().buffers[hit.node.path]?.source ?? "");
      return {
        uri: target.uri,
        range: new monaco.Range(hit.node.line, 1, hit.node.line, target.getLineMaxColumn(hit.node.line)),
      };
    },
  });

  monaco.languages.registerDocumentSymbolProvider(KAI_LANGUAGE, {
    provideDocumentSymbols(model) {
      const outline = appStore().getState().analysis.outlines[pathOfUri(model.uri)];
      if (!outline) return [];
      const symbol = (name: string, detail: string, line: number, kind: monaco.languages.SymbolKind) => {
        const range = new monaco.Range(line, 1, line, model.getLineMaxColumn(Math.min(line, model.getLineCount())));
        return { name, detail, kind, range, selectionRange: range, tags: [] };
      };
      return [
        ...outline.characters.map((c) => symbol(c.id, c.name, c.line, monaco.languages.SymbolKind.Variable)),
        ...outline.labels.map((l) => symbol(l.name, "label", l.line, monaco.languages.SymbolKind.Function)),
      ];
    },
  });

  monaco.languages.registerCompletionItemProvider(KAI_LANGUAGE, {
    triggerCharacters: [" "],
    provideCompletionItems(model, position) {
      const before = model.getLineContent(position.lineNumber).slice(0, position.column - 1);
      const word = model.getWordUntilPosition(position);
      const range = new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn);
      const state = appStore().getState();
      if (/\b(jump|call)\s+\w*$/.test(before)) {
        return {
          suggestions: state.analysis.graph.nodes.map((n) => ({
            label: n.label,
            kind: monaco.languages.CompletionItemKind.Function,
            detail: `${n.path}:${n.line}`,
            insertText: n.label,
            range,
          })),
        };
      }
      if (/^\s*\w*$/.test(before)) {
        const characters = Object.values(state.analysis.outlines).flatMap((o) => o.characters);
        return {
          suggestions: [
            ...characters.map((c) => ({
              label: c.id,
              kind: monaco.languages.CompletionItemKind.Variable,
              detail: c.name,
              insertText: `${c.id} "$0"`,
              insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
              range,
            })),
            ...["scene", "show", "hide", "jump", "call", "menu:", "play music", "pause", "return"].map((k) => ({
              label: k,
              kind: monaco.languages.CompletionItemKind.Keyword,
              insertText: k,
              range,
            })),
          ],
        };
      }
      return { suggestions: [] };
    },
  });

  monaco.editor.registerEditorOpener({
    openCodeEditor(_source, resource, selectionOrPosition) {
      if (resource.scheme !== "kairos") return false;
      const line =
        selectionOrPosition && "startLineNumber" in selectionOrPosition
          ? selectionOrPosition.startLineNumber
          : (selectionOrPosition?.lineNumber ?? 1);
      appStore().getState().openFile(pathOfUri(resource), line);
      return true;
    },
  });
}
