import { Data } from "effect"

export class DbError extends Data.TaggedError("DbError")<{ readonly reason: string }> {}

export class UserNotFound extends Data.TaggedError("UserNotFound")<{ readonly id: string }> {}

export interface User {
  readonly id: string
  readonly name: string
}
