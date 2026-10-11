const fs = require("node:fs")
const path = require("node:path")
const vscode = require("vscode")
const { LanguageClient } = require("vscode-languageclient/node")

let client

function findCompiler() {
  const config = vscode.workspace.getConfiguration("effectscript")
  const configured = config.get("compilerPath") || process.env.EFFECTSCRIPT_COMPILER
  if (configured) {
    return configured
  }
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    let dir = folder.uri.fsPath
    for (;;) {
      for (const name of ["efc", "tsgo"]) {
        const candidate = path.join(dir, "target", "release", name)
        if (fs.existsSync(candidate)) {
          return candidate
        }
      }
      const parent = path.dirname(dir)
      if (parent === dir) {
        break
      }
      dir = parent
    }
  }
  return undefined
}

async function start() {
  if (!vscode.workspace.isTrusted) {
    return
  }
  const compiler = findCompiler()
  if (!compiler) {
    vscode.window.showErrorMessage(
      "EffectScript: efc not found. Build it with `cargo build --release -p ts_goport --bin efc` or set effectscript.compilerPath."
    )
    return
  }
  const documentSelector = [{ scheme: "file", language: "effectscript" }]
  if (vscode.workspace.getConfiguration("effectscript").get("serveTypeScript")) {
    documentSelector.push(
      { scheme: "file", language: "typescript" },
      { scheme: "file", language: "typescriptreact" }
    )
  }
  client = new LanguageClient(
    "effectscript",
    "EffectScript",
    { command: compiler, args: ["--lsp", "--stdio"] },
    { documentSelector }
  )
  await client.start()
}

async function stop() {
  if (client) {
    const running = client
    client = undefined
    await running.stop()
  }
}

async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("effectscript.restartServer", async () => {
      await stop()
      await start()
    }),
    vscode.workspace.onDidGrantWorkspaceTrust(() => start()),
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (event.affectsConfiguration("effectscript")) {
        await stop()
        await start()
      }
    })
  )
  await start()
}

function deactivate() {
  return stop()
}

module.exports = { activate, deactivate }
