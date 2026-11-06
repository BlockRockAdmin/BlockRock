export type GatewayItem = {
  title: string;
  kind: "impegno" | "spesa" | "necessita";
  status: "previsto" | "in_corso" | "concluso";
  note: string;
  amountCents: number | null;
};
export type GatewaySnapshot = { source: string; updatedAt: string; items: GatewayItem[] };

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Formato non valido.");
  return value as Record<string, unknown>;
}
function text(value: unknown, max: number, allowEmpty = false): string {
  if (typeof value !== "string" || value.length > max || (!allowEmpty && !value.trim()) || [...value].some((character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127))
    throw new Error("Testo non valido o troppo lungo.");
  return value.trim();
}
export function parseGateway(raw: string): GatewaySnapshot {
  if (new TextEncoder().encode(raw).length > 1_000_000) throw new Error("Il file supera 1 MB.");
  let decoded: unknown;
  try { decoded = JSON.parse(raw); } catch { throw new Error("Il file non contiene JSON valido."); }
  const data = record(decoded);
  if (data.format !== "blockrock-gateway-v1") throw new Error("Formato del quadro non riconosciuto.");
  const source = text(data.source, 120);
  const updatedAt = text(data.updatedAt, 40);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?Z$/.test(updatedAt) || !Number.isFinite(Date.parse(updatedAt)) || new Date(updatedAt).toISOString().slice(0, 19) !== updatedAt.slice(0, 19)) throw new Error("Data di aggiornamento non valida.");
  if (!Array.isArray(data.items) || data.items.length > 500) throw new Error("Elenco non valido o troppo lungo.");
  const items = data.items.map((value): GatewayItem => {
    const item = record(value);
    if (item.kind !== "impegno" && item.kind !== "spesa" && item.kind !== "necessita") throw new Error("Tipo di voce non valido.");
    if (item.status !== "previsto" && item.status !== "in_corso" && item.status !== "concluso") throw new Error("Stato della voce non valido.");
    if (item.amountCents !== null && (!Number.isSafeInteger(item.amountCents) || (item.amountCents as number) < 0)) throw new Error("Importo non valido.");
    return { title: text(item.title, 160), kind: item.kind, status: item.status, note: text(item.note, 500, true), amountCents: item.amountCents as number | null };
  });
  return { source, updatedAt, items };
}
