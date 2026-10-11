const { Effect } = require("effect")
const { main, read, Config } = require("./out/program.js")

const missing = Effect.runSync(
  read("other").pipe(
    Effect.either,
    Effect.provideService(Config, new Map())
  )
)
if (Effect.runSync(main) !== "effectscript" || missing._tag !== "Left" || missing.left._tag !== "Missing") {
  throw new Error("FAILED: commonjs")
}
console.log("ok - commonjs output")
