export type Step = { id: string; action: string; createdAt: string };
export type Goal = {
  id: string;
  title: string;
  nextStep: string;
  area: "personal" | "stability";
  done: boolean;
};
export type Journey = {
  version: 2;
  paused: boolean;
  steps: Step[];
  goals: Goal[];
  interests: string[];
};
export type Snapshot = { journey: Journey; revision: number; updatedAt: string };
export const legacyKey = "blockrock.recovery-steps.v1";
export const maxFileSize = 2_000_000;
export const emptyJourney = (): Journey => ({
  version: 2,
  paused: false,
  steps: [],
  goals: [],
  interests: [],
});
export const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));

function record(value: unknown, keys: string[]): Record<string, unknown> {
  if (
    !value ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.keys(value).sort().join() !== [...keys].sort().join()
  )
    throw new Error("Formato dei dati non valido.");
  return value as Record<string, unknown>;
}
function text(value: unknown, max: number, allowEmpty = false, multiline = false): string {
  const controls = multiline ? /[\u0000-\u0009\u000b-\u001f]/ : /[\u0000-\u001f]/;
  if (
    typeof value !== "string" ||
    [...value].length > max ||
    [...value].some((character) => {
      const code = character.codePointAt(0)!;
      return code >= 0xd800 && code <= 0xdfff;
    }) ||
    controls.test(value) ||
    (!allowEmpty && !value.trim())
  )
    throw new Error("Testo vuoto, troppo lungo o non valido.");
  return value.trim();
}
function id(value: unknown): string {
  const clean = text(value, 80);
  if (clean !== value || !/^[a-zA-Z0-9_-]+$/.test(clean))
    throw new Error("Identificatore non valido.");
  return clean;
}
export function timestamp(value: unknown): string {
  const date = text(value, 40);
  if (
    date !== value ||
    !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,6})?(?:Z|[+-]\d\d:\d\d)$/.test(date) ||
    !Number.isFinite(Date.parse(date)) ||
    Number(date.slice(0, 4)) < 1970
  )
    throw new Error("Data non valida.");
  const calendar = new Date(date.slice(0, 19) + "Z");
  if (calendar.toISOString().slice(0, 19) !== date.slice(0, 19))
    throw new Error("Data non valida.");
  return date;
}
function list<T>(value: unknown, limit: number, parse: (item: unknown) => T): T[] {
  if (!Array.isArray(value) || value.length > limit)
    throw new Error("Troppi elementi o elenco non valido.");
  return value.map(parse);
}
function unique<T extends { id: string }>(values: T[]): T[] {
  if (new Set(values.map((item) => item.id)).size !== values.length)
    throw new Error("Identificatori duplicati.");
  return values;
}
function step(value: unknown): Step {
  const item = record(value, ["id", "action", "createdAt"]);
  return { id: id(item.id), action: text(item.action, 120), createdAt: timestamp(item.createdAt) };
}
function goal(value: unknown): Goal {
  const item = record(value, ["id", "title", "nextStep", "area", "done"]);
  if ((item.area !== "personal" && item.area !== "stability") || typeof item.done !== "boolean")
    throw new Error("Obiettivo non valido.");
  return {
    id: id(item.id),
    title: text(item.title, 160),
    nextStep: text(item.nextStep, 240, true, true),
    area: item.area,
    done: item.done,
  };
}
export function parseJourney(value: unknown): Journey {
  const item = record(value, ["version", "paused", "steps", "goals", "interests"]);
  if (item.version !== 2 || typeof item.paused !== "boolean")
    throw new Error("Versione del percorso non supportata.");
  const interests = list(item.interests, 50, (value) => text(value, 60));
  if (new Set(interests.map((value) => value.toLowerCase())).size !== interests.length)
    throw new Error("Interessi duplicati.");
  return {
    version: 2,
    paused: item.paused,
    steps: unique(list(item.steps, 10000, step)),
    goals: unique(list(item.goals, 100, goal)),
    interests,
  };
}
export function parseSnapshot(value: unknown): Snapshot {
  const item = record(value, ["journey", "revision", "updatedAt"]);
  if (!Number.isSafeInteger(item.revision) || (item.revision as number) < 0)
    throw new Error("Revisione non valida.");
  return {
    journey: parseJourney(item.journey),
    revision: item.revision as number,
    updatedAt: timestamp(item.updatedAt),
  };
}
export function parseImport(raw: string): Journey {
  if (new TextEncoder().encode(raw).length > maxFileSize)
    throw new Error("File troppo grande: massimo 2 MB.");
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    throw new Error("Il file non contiene JSON valido.");
  }
  if (Array.isArray(data)) return { ...emptyJourney(), steps: unique(list(data, 10000, step)) };
  const envelope = record(data, ["format", "exportedAt", "journey"]);
  if (envelope.format !== "blockrock-nexus") throw new Error("Questo file non è un backup Nexus.");
  timestamp(envelope.exportedAt);
  return parseJourney(envelope.journey);
}
export function exportJourney(journey: Journey): string {
  return JSON.stringify(
    {
      format: "blockrock-nexus",
      exportedAt: new Date().toISOString(),
      journey: parseJourney(journey),
    },
    null,
    2,
  );
}
export function mergeJourney(current: Journey, incoming: Journey): Journey {
  function merge<T extends { id: string }>(a: T[], b: T[]): T[] {
    const result = clone(a);
    for (const item of b) {
      const previous = result.find((value) => value.id === item.id);
      if (!previous) result.push(clone(item));
      else if (JSON.stringify(previous) !== JSON.stringify(item))
        throw new Error(
          "Il backup contiene elementi modificati con lo stesso ID. Usa la sostituzione dopo aver esportato il percorso attuale.",
        );
    }
    return result;
  }
  return parseJourney({
    ...current,
    steps: merge(current.steps, incoming.steps),
    goals: merge(current.goals, incoming.goals),
    interests: [
      ...new Map(
        [...current.interests, ...incoming.interests].map((value) => [value.toLowerCase(), value]),
      ).values(),
    ],
  });
}
export function makeId(): string {
  return crypto.randomUUID();
}
export function download(name: string, content: string) {
  const url = URL.createObjectURL(new Blob([content], { type: "application/json" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
