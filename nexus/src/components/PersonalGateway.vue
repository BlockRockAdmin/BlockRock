<script setup lang="ts">
import { ref } from "vue";
import { parseGateway } from "../domain/gateway";
import type { GatewaySnapshot } from "../domain/gateway";

const snapshot = ref<GatewaySnapshot | null>(null);
const error = ref("");
const kind = { impegno: "Impegno", spesa: "Spesa", necessita: "Necessità" };
const status = { previsto: "Previsto", in_corso: "In corso", concluso: "Concluso" };
const money = new Intl.NumberFormat("it-IT", { style: "currency", currency: "EUR" });
const date = (value: string) => new Intl.DateTimeFormat("it-IT", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
async function choose(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  error.value = "";
  snapshot.value = null;
  try {
    if (file.size > 1_000_000) throw new Error("Il file supera 1 MB.");
    snapshot.value = parseGateway(await file.text());
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : "Impossibile leggere il file.";
  } finally { input.value = ""; }
}
</script>

<template>
  <section class="card" aria-labelledby="gateway-title">
    <p class="eyebrow">Il tuo quadro</p>
    <h1 id="gateway-title">Impegni, spese e necessità</h1>
    <p>Un quadro può aiutarti a vedere cosa occupa le tue risorse e cosa hai dovuto rimandare.</p>
    <p>Per ora puoi aprire un file nel formato BlockRock. Rimane solo in questa pagina: non viene salvato nell'account né inviato al server. Cambiando pagina o ricaricando dovrai riaprirlo.</p>
    <p>Cecchina Hub e gli altri gateway non sono ancora collegati automaticamente.</p>
    <label>Scegli un file del quadro
      <input type="file" accept="application/json,.json" @change="choose" />
    </label>
    <p v-if="error" class="error-box" role="alert">{{ error }}</p>
    <template v-if="snapshot">
      <p role="status">Fonte dichiarata: <strong>{{ snapshot.source }}</strong> · Aggiornato: {{ date(snapshot.updatedAt) }}. Verifica che i dati siano ancora attuali.</p>
      <button @click="snapshot = null">Chiudi il quadro</button>
      <p v-if="!snapshot.items.length">Nessuna voce nel file.</p>
      <div v-for="(item, index) in snapshot.items" :key="index" class="card">
        <h2>{{ item.title }}</h2>
        <p>{{ kind[item.kind] }} · {{ status[item.status] }}<template v-if="item.amountCents !== null"> · {{ money.format(item.amountCents / 100) }}</template></p>
        <p v-if="item.note">{{ item.note }}</p>
      </div>
    </template>
  </section>
</template>
