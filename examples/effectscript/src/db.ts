import { Context, Effect, Layer } from "effect"
import { DbError, type User } from "./errors.js"

export class Database extends Context.Tag("Database")<
  Database,
  {
    readonly findUser: (id: string) => Effect.Effect<User | undefined, DbError>
  }
>() {}

export class Logger extends Context.Tag("Logger")<
  Logger,
  {
    readonly info: (message: string) => Effect.Effect<void>
  }
>() {}

const users = new Map<string, User>([
  ["1", { id: "1", name: "Ada" }],
  ["2", { id: "2", name: "Grace" }]
])

export const DatabaseLive = Layer.succeed(Database, {
  findUser: (id) =>
    id === "broken"
      ? Effect.fail(new DbError({ reason: "connection lost" }))
      : Effect.succeed(users.get(id))
})

export const logged: Array<string> = []

export const LoggerLive = Layer.succeed(Logger, {
  info: (message) => Effect.sync(() => void logged.push(message))
})

export function formatName(user: User): string {
  return `${user.name} (#${user.id})`
}
