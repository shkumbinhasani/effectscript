import { readFileSync } from "node:fs"
import { createRequire } from "node:module"
import path from "node:path"

const [modules, tsGrammarPath, file, filter] = process.argv.slice(2)
if (!modules || !tsGrammarPath || !file) {
  console.log("usage: node test/tokenize.mjs <dir with vscode-textmate> <TypeScript.tmLanguage.json> <file.ef> [line filter]")
  process.exit(2)
}
const require = createRequire(path.join(path.resolve(modules), "x.js"))
const textmate = require("vscode-textmate")
const oniguruma = require("vscode-oniguruma")
const here = path.dirname(new URL(import.meta.url).pathname)

await oniguruma.loadWASM(readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm")).buffer)
const grammars = {
  "source.ef": path.join(here, "../syntaxes/effectscript.tmLanguage.json"),
  "source.ef.keywords": path.join(here, "../syntaxes/effectscript.keywords.tmLanguage.json"),
  "source.ts": tsGrammarPath
}
const registry = new textmate.Registry({
  onigLib: Promise.resolve({
    createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
    createOnigString: (text) => new oniguruma.OnigString(text)
  }),
  loadGrammar: async (scope) =>
    grammars[scope] ? textmate.parseRawGrammar(readFileSync(grammars[scope], "utf8"), grammars[scope]) : null,
  getInjections: (scope) => (scope === "source.ef" ? ["source.ef.keywords"] : [])
})
const grammar = await registry.loadGrammar("source.ef")
let stack = textmate.INITIAL
const lines = readFileSync(file, "utf8").split("\n")
lines.forEach((line, index) => {
  const result = grammar.tokenizeLine(line, stack)
  stack = result.ruleStack
  if (filter && !new RegExp(filter).test(line)) return
  const parts = result.tokens
    .map((t) => [line.slice(t.startIndex, t.endIndex), t.scopes.slice(1).filter((s) => !/^meta\.(block|function|var\.expr|definition\.variable)/.test(s)).join(" ")])
    .filter(([text]) => text.trim())
    .map(([text, scopes]) => `${JSON.stringify(text)}=${scopes.split(" ").pop() || "-"}`)
  console.log(`${String(index + 1).padStart(3)}: ${parts.join("  ")}`)
})
