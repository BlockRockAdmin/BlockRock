import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { test } from "node:test";
import ts from "typescript";

// Esegue il modulo TypeScript reale con il runner standard di Node.
const source = await readFile(new URL("../src/domain/journey.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
});
const {
  emptyJourney,
  parseJourney,
  parseImport,
  exportJourney,
  mergeJourney,
  parseSnapshot,
  timestamp,
} = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
const step = { id: "step-1", action: "Sono uscito", createdAt: "2026-11-05T12:00:00.000Z" };
const goal = {
  id: "goal-1",
  title: "Suonare",
  nextStep: "Dieci minuti",
  area: "personal",
  done: false,
};

test("export/import conserva tutti i dati e lo stato della pausa", () => {
  const journey = {
    ...emptyJourney(),
    paused: true,
    steps: [step],
    goals: [goal],
    interests: ["Musica"],
  };
  assert.deepEqual(parseImport(exportJourney(journey)), journey);
});
test("migrazione del prototipo conserva identificatori e date", () => {
  assert.deepEqual(parseImport(JSON.stringify([step])), { ...emptyJourney(), steps: [step] });
});
test("nessuna migrazione parziale silenziosa di elementi corrotti", () => {
  assert.throws(() =>
    parseImport(JSON.stringify([step, { ...step, id: "other", createdAt: "non una data" }])),
  );
});
test("date impossibili, senza fuso e prima del 1970 sono rifiutate", () => {
  for (const value of [
    "2026-02-30T12:00:00Z",
    "2026-11-05",
    "2026-11-05T12:00:00",
    "1969-01-01T00:00:00Z",
    "2026-11-05T24:00:00Z",
  ])
    assert.throws(() => timestamp(value), value);
  assert.equal(timestamp("2026-11-05T12:00:00+01:00"), "2026-11-05T12:00:00+01:00");
});
test("file, versioni o campi sconosciuti sono rifiutati", () => {
  for (const value of [
    "broken",
    "{}",
    JSON.stringify({ ...emptyJourney(), version: 7 }),
    JSON.stringify({ format: "other", exportedAt: step.createdAt, journey: emptyJourney() }),
  ])
    assert.throws(() => parseImport(value));
  assert.throws(() => parseJourney({ ...emptyJourney(), userId: "other" }));
});
test("limite di importazione misurato in byte UTF-8", () => {
  assert.throws(() => parseImport("à".repeat(1_000_001)), /troppo grande/);
});
test("non accetta coercizioni di booleani o revisioni", () => {
  assert.throws(() => parseJourney({ ...emptyJourney(), paused: "false" }));
  assert.throws(() =>
    parseSnapshot({ journey: emptyJourney(), revision: "1", updatedAt: step.createdAt }),
  );
});
test("identificatori duplicati e interessi equivalenti sono rifiutati", () => {
  assert.throws(() => parseJourney({ ...emptyJourney(), steps: [step, step] }));
  assert.throws(() => parseJourney({ ...emptyJourney(), goals: [goal, goal] }));
  assert.throws(() => parseJourney({ ...emptyJourney(), interests: ["Musica", "musica"] }));
});
test("testi vuoti, caratteri di controllo e ID pericolosi sono rifiutati", () => {
  for (const action of [" ", "a".repeat(121), "test\u0000"])
    assert.throws(() => parseJourney({ ...emptyJourney(), steps: [{ ...step, action }] }));
  assert.throws(() => parseJourney({ ...emptyJourney(), steps: [{ ...step, id: "../x" }] }));
});
test("campi obiettivo e confini delle aree sono validati", () => {
  assert.throws(() => parseJourney({ ...emptyJourney(), goals: [{ ...goal, area: "risk" }] }));
  assert.throws(() => parseJourney({ ...emptyJourney(), goals: [{ ...goal, done: 1 }] }));
  assert.deepEqual(
    parseJourney({ ...emptyJourney(), goals: [{ ...goal, area: "stability" }] }).goals[0].area,
    "stability",
  );
});
test("unione idempotente: non duplica elementi, conserva la pausa attuale", () => {
  const current = { ...emptyJourney(), paused: true, steps: [step], interests: ["Musica"] };
  const incoming = {
    ...emptyJourney(),
    steps: [step, { ...step, id: "step-2" }],
    goals: [goal],
    interests: ["musica", "Cucina"],
  };
  const merged = mergeJourney(current, incoming);
  assert.equal(merged.steps.length, 2);
  assert.equal(merged.paused, true);
  assert.equal(merged.interests.length, 2);
  assert.deepEqual(mergeJourney(merged, incoming), merged);
  assert.equal(current.steps.length, 1);
});
test("unione di due versioni dello stesso elemento richiede scelta esplicita", () => {
  const current = { ...emptyJourney(), steps: [step] };
  assert.throws(
    () => mergeJourney(current, { ...emptyJourney(), steps: [{ ...step, action: "Modificato" }] }),
    /stesso ID/,
  );
  assert.equal(current.steps[0].action, "Sono uscito");
});
test("nessuna mutazione condivisa dopo unione o parsing", () => {
  const original = { ...emptyJourney(), steps: [step] };
  const merged = mergeJourney(original, emptyJourney());
  merged.steps[0].action = "Nuovo";
  assert.equal(original.steps[0].action, "Sono uscito");
});
test("limiti di archiviazione non vengono superati dopo unione", () => {
  const current = {
    ...emptyJourney(),
    interests: Array.from({ length: 50 }, (_, n) => `interesse-${n}`),
  };
  assert.throws(() => mergeJourney(current, { ...emptyJourney(), interests: ["ancora uno"] }));
  assert.throws(() =>
    parseJourney({
      ...emptyJourney(),
      goals: Array.from({ length: 101 }, (_, n) => ({ ...goal, id: `goal-${n}` })),
    }),
  );
});

test("testi Unicode e prossimo passo su più righe rimangono esportabili", () => {
  const journey = {
    ...emptyJourney(),
    steps: [{ ...step, action: "🌱".repeat(100) }],
    goals: [{ ...goal, nextStep: "Prima un gesto\nPoi scelgo il resto" }],
    interests: ["Straße", "STRASSE"],
  };
  assert.deepEqual(parseImport(exportJourney(journey)), journey);
});
