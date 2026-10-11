import { Effect, Exit, Layer } from "effect"
import { DatabaseLive, LoggerLive, logged } from "./db.js"
import { add, addLater, answer, bodyRuns, findOrGuest, getName, getNames, getUser, orDefault } from "./users.js"
import type { DbError, User, UserNotFound } from "./errors.js"
import type { Database } from "./db.js"

function check(label: string, ok: boolean) {
  if (!ok) {
    throw new Error(`FAILED: ${label}`)
  }
  console.log(`ok - ${label}`)
}

const typed: Effect.Effect<User, UserNotFound | DbError, Database> = getUser("1")

const sum: number = add(2, 3)
check("ordinary functions stay ordinary", sum === 5)
check("async functions still return promises", addLater(1, 2) instanceof Promise && (await addLater(1, 2)) === 3)

check("calling an effect function runs nothing", bodyRuns === 0 && Effect.isEffect(typed))

const program = getUser("1").pipe(Effect.provide(DatabaseLive))
const user = await Effect.runPromise(program)
check("use and await acquire the service and compose effects", user.name === "Ada" && bodyRuns === 1)

const missing = await Effect.runPromise(
  getUser("404").pipe(
    Effect.catchTag("UserNotFound", (e) => Effect.succeed(`missing ${e.id}`)),
    Effect.provide(DatabaseLive)
  )
)
check("throw becomes a typed, recoverable failure", missing === "missing 404")

const exit = await Effect.runPromiseExit(getUser("broken").pipe(Effect.provide(DatabaseLive)))
check(
  "awaited failures propagate through the error channel",
  Exit.isFailure(exit) && exit.cause._tag === "Fail" && exit.cause.error._tag === "DbError"
)

const AppLive = Layer.merge(DatabaseLive, LoggerLive)
const names = await Effect.runPromise(getNames(["1", "2"]).pipe(Effect.provide(AppLive)))
check("chained effect functions", names.join(",") === "Ada (#1),Grace (#2)" && logged.join(",") === "loaded 1,loaded 2")

check("effect function with no clauses", (await Effect.runPromise(answer())) === 42)

const fallback = await Effect.runPromise(orDefault(Effect.fail("boom"), 7))
check("generic effect functions", fallback === 7)

const guest = await Effect.runPromise(findOrGuest("9").pipe(Effect.provide(DatabaseLive)))
check("inferred channels", guest.name === "guest")
