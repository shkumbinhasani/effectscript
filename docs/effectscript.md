# EffectScript

EffectScript is TypeScript with effect functions built into the language. An
EffectScript file has the extension `.ef`. It accepts all TypeScript syntax,
plus three constructs:

- `effect async function` declarations, with `requires` and `throws` clauses
- `use` expressions, which acquire an Effect service
- `await` and `throw` inside an effect function, which compose Effects

The compiler turns an effect function into an ordinary function that returns
an `Effect.Effect` from the [`effect`](https://effect.website) npm package.
It does not have a runtime of its own: the `effect` package runs the result.

```ts
// users.ef
import { Database } from "./db.js"
import { DbError, UserNotFound, type User } from "./errors.js"

export effect async function getUser(id: string): User
  requires Database
  throws UserNotFound | DbError
{
  const db = use Database
  const user = await db.findUser(id)

  if (!user) throw new UserNotFound({ id })

  return user
}
```

`getUser` has the type
`(id: string) => Effect.Effect<User, UserNotFound | DbError, Database>`.
Calling it runs nothing. It runs when the Effect runs:

```ts
// main.ts
import { Effect } from "effect"
import { DatabaseLive } from "./db.js"
import { getUser } from "./users.js"

const program = getUser("123").pipe(Effect.provide(DatabaseLive))
const user = await Effect.runPromise(program)
```

## Build and run

The compiler is `efc`. It is the TypeScript compiler of this repository with EffectScript built in; the same binary is also built as `tsgo`, the name the repository's own tooling uses.

```sh
cargo build --release -p ts_goport --bin efc
cd examples/effectscript
npm ci
../../target/release/efc -p tsconfig.json    # writes dist/*.js and dist/*.d.ts
node dist/main.js                            # runs the example
npm run consumer                             # plain tsc 5.9 reads dist/*.d.ts
```

The tests:

```sh
cargo test --release -p ts_goport --test effectscript
```

They need `npm ci` in `examples/effectscript` first, because they type-check
against the installed `effect` declarations and run the output with Node.js.
Without it they fail and say so.

## Editor support (VS Code)

`editors/vscode-effectscript` is a VS Code extension. It registers the `.ef`
language, highlights `effect`, `use`, `requires` and `throws` on top of the
TypeScript grammar, and runs `efc --lsp --stdio` as the language server.

```sh
cargo build --release -p ts_goport --bin efc
cd editors/vscode-effectscript
npm ci
npx @vscode/vsce package --allow-missing-repository --skip-license -o effectscript.vsix
code --install-extension effectscript.vsix
```

Open `examples/effectscript` in VS Code. The extension finds
`target/release/efc` in the workspace folder or a parent of it; set
`effectscript.compilerPath` to use another binary. After rebuilding `efc`, run
"EffectScript: Restart Language Server".

Diagnostics, hover, go to definition, references, completions, signature
help, folding, document symbols and inlay hints work in `.ef` files.
`scripts/effectscript/lsp-sweep.mjs` sends each of these requests at every
position of a file and reports any error or hang; it runs clean on the
example, on the diagnostic cases and on half-typed code.

Formatting returns no edits for `.ef` files: the formatter works from the
lowered tree and would reflow code around nodes the user did not write.

`.ts` files keep VS Code's built-in TypeScript service. That service does not
know `.ef` files, so in a `.ts` file an import of a `.ef` module shows
"Cannot find module" in the editor, though `efc -p` compiles it. Setting
`effectscript.serveTypeScript` sends `.ts` files to `efc` as well; turn off
the built-in TypeScript extension for the workspace when you use it.

## The language

### Effect functions

```text
EffectFunctionDeclaration:
  Modifiers? effect async function BindingIdentifier TypeParameters? Parameters
    (: SuccessType)?
    (requires Type)?
    (throws Type)?
    FunctionBody

UseExpression:
  use UnaryExpression
```

- `effect` comes first and must be followed, on the same line, by `async`.
  `effect function` without `async` is an error (TS378008). `export` and
  `declare` work as usual.
- The annotation after `:` is the success type, not the return type. The
  function returns `Effect.Effect<Success, Errors, Requirements>`.
- `requires` lists the services the body needs. Use a union for more than
  one: `requires Database | Logger`.
- `throws` lists the recoverable errors.
- A missing `requires` or `throws` clause means `never`. So
  `effect async function answer(): number { return 42 }` has the type
  `() => Effect.Effect<number, never, never>`.
- Without a success type and without clauses, all three channels are
  inferred from the body (see "Inferred effect functions"). A clause without
  a success type is an error (TS378009).
- A body that is only `;` declares an overload or an ambient function, as in
  TypeScript.

`effect`, `use`, `requires` and `throws` are contextual words. They keep
their meaning as identifiers everywhere else, so `const effect = 1` and
`use(x)` still work in a `.ef` file.

### Inside an effect function

| Source | Meaning | Effect equivalent |
|---|---|---|
| `await e` | Run the Effect `e` and give its success value | `yield* e` |
| `use Tag` | Get the service of `Tag` from the context | `yield* Tag` |
| `throw e` | Fail with the typed error `e` | `return yield* Effect.fail(e)` |
| `return v` | Succeed with `v` | `return v` |

The body runs inside `Effect.gen`, so it keeps Effect's behavior for
interruption, finalizers, fibers and supervision.

`await`, `use` and `throw` belong to the nearest function. In a nested
callback or a nested `async` function inside an effect function they keep
their TypeScript meaning: `await` waits for a Promise and `throw` throws.

### Errors and defects

- A `throw` statement written in the body of an effect function is a
  recoverable failure in the error channel. `Effect.catchTag`,
  `Effect.catchAll` and `Effect.either` see it.
- An exception that escapes from ordinary JavaScript code that the body
  calls (a `.ts` function that throws, a bug, a failed property read) is a
  defect, as it is in `Effect.gen`. It is not in the error channel.
- An awaited Effect that fails makes the effect function fail with the same
  error.

### try, catch and finally

`try` works on typed failures. The `catch` variable gets the union of the
errors that the `await`s and `throw`s in the `try` block can produce, not
`unknown`:

```ts
effect async function findOrGuest(id: string): string
  throws DbError
{
  try {
    return await find(id)              // can fail with NotFound | DbError
  } catch (e) {                        // e: NotFound | DbError
    if (e._tag === "NotFound") return `guest ${e.id}`
    throw e                            // DbError goes back to the error channel
  } finally {
    log.push(`done ${id}`)
  }
}
```

- The checker tests the `catch` block against the clauses, so the `throws`
  clause only needs the errors that the `catch` lets through. Above, the
  `catch` handles `NotFound`, so `throws DbError` is enough.
- `catch` sees typed failures only, as `Effect.catchAll` does. A defect
  (an exception from plain JavaScript code in the `try` block) passes the
  `catch` and stays a defect.
- `finally` runs after success, after a failure and after interruption
  (`Effect.ensuring`). It cannot fail: an `await` or `throw` there whose
  errors are not handled is error TS378013. It cannot `return` either
  (TS378014).
- `return` in a `try` or `catch` block leaves the effect function, as in
  JavaScript. `break` and `continue` work for a loop around the `try`.
  `break` and `continue` with a label that leave the `try` are error
  TS378011.
- Without a success type, the success type is inferred from every `return`,
  those in the `try` and `catch` blocks included.

The compiler turns the `try` block into its own Effect:

```js
const __es_c1 = yield* __es_Effect.ensuring(
  __es_Effect.catchAll(
    __es_Effect.gen(function* () { return { v: (yield* find(id)) } }),
    (e) => __es_Effect.gen(function* () {
      if (e._tag === "NotFound") return { v: `guest ${e.id}` }
      return yield* __es_Effect.fail(e)
    })),
  __es_Effect.orDie(__es_Effect.gen(function* () { log.push(`done ${id}`) })))
if (__es_c1) return __es_c1.v
```

A `return` in the block finishes it with `{ v: value }`, a `break` with
`{ b: true }` and a `continue` with `{ k: true }`. After the block, the
function returns the value, or breaks or continues the loop, the way
TypeScript lowers `async` functions for old JavaScript targets.

### Promises

`await` in an effect function takes Effects only. Awaiting a Promise is an
error (TS378004). Convert the Promise with `Effect.promise` (it cannot fail)
or `Effect.tryPromise` (it can). Awaiting a value that is neither a Promise
nor an Effect is error TS378005.

### Checked contracts

With a success type, the declaration is a contract that the checker
enforces:

- A `return` value must be assignable to the success type (TS2322 at the
  `return`).
- Each awaited Effect may fail only with errors in the `throws` clause, or
  TS378001 names the undeclared errors at the `await`.
- Each awaited Effect and each `use` may need only services in the
  `requires` clause, or TS378002 names the undeclared services.
- Each thrown value must be in the `throws` clause, or TS378003 names its
  type.

To await an Effect with more errors than the clause allows, handle them
first, for example `await Effect.catchAll(db.find, () => Effect.succeed(x))`.
The checker uses the types of the installed `effect` declarations, so it sees
exactly the error and requirement channels that `Effect.catchAll` and the
other combinators produce.

### Inferred effect functions

```ts
effect async function findOrGuest(id: string) {
  const db = use Database
  const user = await db.findUser(id)
  return user ?? { id, name: "guest" }
}
```

With no success type and no clauses, the compiler infers all three channels
the way `Effect.gen` does. The type above is
`(id: string) => Effect.Effect<User, DbError, Database>`. The `effect`
keyword is still required: the compiler never treats a function as an
effect function because of what its body contains.

### Generics

Type parameters work as in TypeScript, and a clause can name them:

```ts
effect async function orDefault<A, E, R>(effect: Effect.Effect<A, E, R>, fallback: A): A
  requires R
{
  return await Effect.orElseSucceed(effect, () => fallback)
}
```

## Effect modules need no import

In a `.ef` file the common modules of `effect` are in scope without an
import, the way `Promise` is in scope in TypeScript:

```text
Effect Context Layer Data Option Either Exit Cause Schedule Duration Stream
Schema Fiber Chunk HashMap HashSet Deferred PubSub Redacted DateTime Brand
Metric ManagedRuntime Runtime STM Micro Match Console pipe
```

```ts
// errors.ef: no imports
export class Missing extends Data.TaggedError("Missing")<{ readonly key: string }> {}
export class Config extends Context.Tag("Config")<Config, ReadonlyMap<string, string>>() {}
export const ConfigLive = Layer.succeed(Config, new Map([["name", "effectscript"]]))
```

The compiler adds `import { Effect, Context, Layer, Data } from "effect"`
with the names the file uses. A name that the file declares or imports at
the top level is not added, so your own `Option` type or a
`import { Effect as E }` keeps working. The list leaves out the `effect`
modules named like JavaScript or TypeScript globals (`Array`, `Number`,
`String`, `Function`, `Symbol`, `Record`) and generic names (`Logger`,
`Config`, `Random`, `Clock`, `Ref`, `Scope`, `Queue`); import those as usual.

The `effect` package still has to be installed. `Promise` is part of every
JavaScript engine; Effect's runtime is a library, so the emitted JavaScript
imports it from `node_modules` and the checker reads its types from there.

## Output

The output reads like hand-written Effect code. `getUser` above becomes:

```js
import { Effect } from "effect";

export function getUser(id) {
    return Effect.gen(function* () {
        const db = (yield* Database);
        const user = (yield* db.findUser(id));
        if (!user)
            return yield* Effect.fail(new UserNotFound({ id }));
        return user;
    });
}
```

and its declaration is:

```ts
import { Effect } from "effect";
export declare function getUser(id: string): Effect.Effect<User, UserNotFound | DbError, Database>;
```

The compiler imports `Effect` itself when the file does not. If the file
declares something else named `Effect` anywhere, even a parameter in a
callback, the generated code uses a private
`import { Effect as __es_Effect } from "effect"` instead, so your name cannot
break it. The declaration files are plain TypeScript:
`examples/effectscript/consumer` is checked by the `tsc` 5.9 package with
`skipLibCheck` off. ES module and CommonJS output both work. A `.ef` file
without effect functions emits byte for byte what the same `.ts` file emits.

## Files and modules

- `.ef` is a TypeScript source extension. `tsconfig.json` globs include it,
  and `tsc file.ef` accepts it.
- `import "./users.js"` and `import "./users"` find `users.ef` the way they
  find `users.ts`. When both exist, `users.ts` wins, as `.ts` wins over
  `.tsx`.
- `users.ef` emits `users.js` and `users.d.ts`. A `.ts` file and a `.ef`
  file with the same name in one directory would write the same output, and
  the compiler reports TS5056.
- `.ts`, `.tsx`, `.js` and `.jsx` files are unchanged. `effect async
  function` and `use` are syntax errors there.
- Messages that list the supported extensions (TS6054, TS6231) leave `.ef`
  out, so their text is the same as before for TypeScript projects. Module
  resolution does probe for `.ef` files, so `--traceResolution` output shows
  those lookups.

## Regression testing

`scripts/effectscript/differential.py` compiles each given file with two
compiler binaries and compares the diagnostics, exit code and every emitted
`.js` and `.d.ts` byte for byte:

```sh
scripts/effectscript/differential.py <main-tsgo> target/release/efc /tmp/out \
  ../TypeScript/tests/cases/compiler/*.ts
```

Against `main`, all 6,401 files in TypeScript's `tests/cases/compiler` and all
5,902 files in `tests/cases/conformance` give the same result. The
`ts_goport` library tests also give the same result as `main`.

## Diagnostics

| Code | Message |
|---|---|
| 378001 | Effect can fail with '{0}', which is not declared in the 'throws' clause of '{1}'. |
| 378002 | Effect requires '{0}', which is not declared in the 'requires' clause of '{1}'. |
| 378003 | Thrown value of type '{0}' is not declared in the 'throws' clause of '{1}'. |
| 378004 | A Promise cannot be awaited in an effect function. Convert it with 'Effect.promise' or 'Effect.tryPromise'. |
| 378005 | Expression of type '{0}' is not an Effect and cannot be awaited in an effect function. |
| 378006 | A 'use' expression is only allowed in the body of an effect function. |
| 378007 | '{0}' is not supported in effect functions yet. (`this`, `arguments`, `for await`) |
| 378008 | An 'effect' function must be declared 'async'. |
| 378009 | An effect function with a 'requires' or 'throws' clause must declare its success type. |
| 378010 | An effect function cannot be a generator. |
| 378011 | '{0}' cannot jump out of a 'try', 'catch' or 'finally' block in an effect function yet. |
| 378012 | Expression of type '{0}' is not an Effect service and cannot be used with 'use'. |
| 378013 | A 'finally' block in an effect function cannot fail. Handle '{0}' inside the block. |
| 378014 | A 'return' statement cannot be used in a 'finally' block of an effect function. |

`examples/effectscript/cases/semantic` and `cases/syntax` show each of them.

## How the compiler does it

The compiler lowers an effect function while it parses it, then checks and
emits the lowered tree with the existing checker and emitter. The lowered
tree is ordinary TypeScript, so the checker infers and checks it with the
real `effect` declarations: there is no second type system and no `any`.

1. **Extension.** `.ef` is in the extension tables
   (`frontend/tspath/extension.rs` and its copies in
   `modulespecifiers/tspath_extension.rs`, `checker/checker_p17.rs`,
   `checker/utilities_p2.rs` and `binder/binder_p1.rs`). Its script kind is
   TS (`frontend/core_ext.rs`). Module resolution tries `.ef` after `.ts`
   (`frontend/module/resolver_p2.rs`), and output names map it to `.js` and
   `.d.ts`.
2. **Parser** (`frontend/parser/effect_lowering.rs`, hooks in
   `parser_p1.rs` to `parser_p5.rs`). The hooks run only when the file name
   ends in `.ef`. The parser keeps a stack of function bodies, so it knows
   whether the nearest function is an effect function.
   - `effect async function f(...): A requires R throws E { body }` becomes
     `function f(...): __es_Effect.Effect<A, E, R> { return __es_Effect.gen(function* (): Generator<__es_YieldWrap<__es_Effect.Effect<unknown, E, R>>, A, never> { body }) }`.
     The annotation on the generator makes the checker test each `return`
     against `A` and each `yield*` against the declared channels.
   - `await e` and `use T` become `(yield* e)` and `(yield* T)`. `throw e`
     becomes `return yield* __es_Effect.fail(e)`.
   - The file gets `import { Effect as __es_Effect } from "effect"` and, for
     a function with a success type,
     `import type { YieldWrap as __es_YieldWrap } from "effect/Utils"`.
   - The new nodes that hold user code keep the source range of that code,
     so diagnostics point at the user's `await`, `use` or `throw`. The new
     names (`__es_Effect`, `gen`, `fail`) have no source range, so the
     printer writes their text instead of slicing the source.
3. **Checker** (`checker/effectscript.rs`, a hook in
   `check_yield_expression` in `checker/checker_p12.rs`). A `yield*` that
   came from `await`, `use` or `throw` is found by the keyword in the source
   at its position. For these the checker reports the Promise and non-Effect
   errors, and it replaces the generic "not assignable to YieldWrap<...>"
   error with the contract errors 378001 to 378003. It reads the error and
   requirement channels of a type from the `_E` and `_R` members of Effect's
   variance struct, so it works for `Effect`, `Context.Tag` and any other
   type that extends `Effect`.
4. **Diagnostics** (`diagnostics/effectscript_catalog.rs`,
   `effectscript/diag.rs`). Codes 378000 to 378999, next to the Effect
   language service codes (377xxx).
5. **Declarations.** The emitter writes the annotation
   `Effect.Effect<A, E, R>` (or `__es_Effect.Effect<A, E, R>`) as it is. An inferred effect function gets
   its type from the checker, which names the added import
   (`scanner_util.rs` `declaration_name_to_string` now prints the text of a
   name without a source range instead of "(Missing)").

### Trade-offs

- **Parse-time lowering.** The checker and emitter see an ordinary
  TypeScript tree, which kept the change small. The cost shows only when a
  file declares its own `Effect`: then generated code and inferred types
  use `__es_Effect`.
- **`async` is required but the function is not a JavaScript async
  function.** The `async` keyword marks where `await` may appear; `effect`
  decides what `await` means. Plain `async function` in a `.ef` file still
  returns a Promise.
- **`throw` is a typed failure only where it is written.** The compiler
  cannot know which JavaScript calls may throw, so exceptions from calls
  stay defects. This matches `Effect.gen`.
- **`try` catches typed failures, not defects.** In `Effect.gen` a failed
  `yield*` does not reach a JavaScript `catch` block, so the compiler turns
  each `try` into `Effect.catchAll` and `Effect.ensuring`. Catching defects
  too would give the `catch` variable the type `unknown` and lose the
  typed errors.

## Limits

- Effect 3 only. The lowering uses `Effect.gen`, `Effect.fail` and
  `YieldWrap` from `effect/Utils`, which have those names and shapes in
  `effect` 3.x (tested with 3.22.2). Effect 4 changed the generator types
  and renamed `Context.Tag` to `Context.Service`.
- Only function declarations. Effect methods, effect arrow functions,
  effect function expressions and `export default effect async function`
  are not parsed yet.
- `for await`, `this` and `arguments` are errors in an effect function, and
  so are labeled `break` and `continue` out of a `try` block.
- `await` of a Promise is an error; there is no implicit conversion.
- The checker finds the channels of a type through Effect's variance
  struct (`_A`, `_E`, `_R`). Every type that extends `Effect` has one,
  including `Context.Tag`, `Option` and `Either`. A yieldable type without
  one gives TS378005.
- In the editor, completions do not offer `use`, `requires` or `throws`,
  and formatting is off for `.ef` files.
- The automatic import does not follow scopes: a name in the list that is
  used only as a local variable still gets imported. TypeScript drops
  unused imports from the JavaScript, and the compiler never reports an
  added import as unused, so this has no visible effect.
- `.efx` (EffectScript with JSX) is not implemented.

## Next steps

1. Lower `for await` to `Stream`, and labeled jumps out of `try` blocks.
2. Effect methods, arrow functions and expressions (`effect async () => ...`).
3. Effect 4 support: choose the lowering from the installed `effect`
   version.
4. Language service: completions for `use` and clause keywords, and quick
   fixes that add a missing error or service to a clause.
5. Infer only the clauses that are left out when a success type is given.
