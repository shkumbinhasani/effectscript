import { Effect, Layer } from "effect"
import type { Database, Logger } from "../dist/db.js"
import { DatabaseLive, LoggerLive } from "../dist/db.js"
import type { DbError, User, UserNotFound } from "../dist/errors.js"
import { answer, getName, getUser } from "../dist/users.js"

const user: Effect.Effect<User, UserNotFound | DbError, Database> = getUser("1")
const name: Effect.Effect<string, UserNotFound | DbError, Database | Logger> = getName("2")
const fortyTwo: Effect.Effect<number> = answer()

const program = Effect.all([user, name, fortyTwo]).pipe(
  Effect.provide(Layer.merge(DatabaseLive, LoggerLive))
)

const [loaded, loadedName, value] = await Effect.runPromise(program)
if (loaded.name !== "Ada" || loadedName !== "Grace (#2)" || value !== 42) {
  throw new Error("FAILED: consumer")
}
console.log("ok - plain TypeScript consumer reads the emitted declarations")
