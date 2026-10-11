declare const Database: unknown

effect async function getUser(): number {
  const db = use Database
  return 1
}
