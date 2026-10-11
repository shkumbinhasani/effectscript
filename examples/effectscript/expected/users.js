import { Effect } from "effect";
import { Effect as __es_Effect } from "effect";
import { Database, Logger, formatName } from "./db.js";
import { UserNotFound } from "./errors.js";
export function add(a, b) {
    return a + b;
}
export async function addLater(a, b) {
    return a + b;
}
export let bodyRuns = 0;
export function getUser(id) {
    return __es_Effect.gen(function* () {
        bodyRuns++;
        const db = (yield* Database);
        const user = (yield* db.findUser(id));
        if (!user)
            return yield* __es_Effect.fail(new UserNotFound({ id }));
        return user;
    });
}
export function getName(id) {
    return __es_Effect.gen(function* () {
        const logger = (yield* Logger);
        const user = (yield* getUser(id));
        (yield* logger.info(`loaded ${user.id}`));
        return formatName(user);
    });
}
export function getNames(ids) {
    return __es_Effect.gen(function* () {
        const names = [];
        for (const id of ids) {
            names.push((yield* getName(id)));
        }
        return names;
    });
}
export function answer() {
    return __es_Effect.gen(function* () {
        return 42;
    });
}
export function orDefault(effect, fallback) {
    return __es_Effect.gen(function* () {
        return (yield* Effect.orElseSucceed(effect, () => fallback));
    });
}
export function findOrGuest(id) {
    return __es_Effect.gen(function* () {
        const db = (yield* Database);
        const user = (yield* db.findUser(id));
        return user ?? { id, name: "guest" };
    });
}
