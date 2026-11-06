import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/domain/gateway.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } });
const { parseGateway } = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
const example = { format: "blockrock-gateway-v1", source: "Personale", updatedAt: "2026-11-06T10:00:00Z", items: [{title: "Acquisto", kind: "impegno", status: "previsto", note: "Rimborso atteso", amountCents: 20000}] };

test("mantiene distinti impegni, stato e fonte senza trasformarli in saldo", () => {
  const result = parseGateway(JSON.stringify(example));
  assert.equal(result.source, "Personale");
  assert.equal(result.items[0].status, "previsto");
  assert.equal(result.items[0].amountCents, 20000);
});
test("rifiuta file parzialmente invalidi e date impossibili", () => {
  for (const input of [
    {...example, updatedAt: "2026-02-30T10:00:00Z"},
    {...example, format: "wrong"},
    {...example, items: [example.items[0], {...example.items[0], amountCents: -1}]},
    {...example, items: [example.items[0], {...example.items[0], status: "pagato"}]},
  ]) assert.throws(() => parseGateway(JSON.stringify(input)));
});
test("accetta importi sconosciuti e rifiuta file troppo grandi", () => {
  assert.equal(parseGateway(JSON.stringify({...example, items: [{...example.items[0], amountCents: null}]})).items[0].amountCents, null);
  assert.throws(() => parseGateway(" ".repeat(1_000_001)));
});
