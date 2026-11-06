<script setup lang="ts">
import { onMounted, ref } from "vue";
import { download, legacyKey, maxFileSize, mergeJourney, parseImport } from "../domain/journey";
import type { Journey } from "../domain/journey";
const props = defineProps<{
  journey: Journey;
  canEdit: boolean;
  busy: boolean;
  username: string;
  exportData: () => void;
  change: (update: (value: Journey) => void, announcement: string) => Promise<void>;
  changePassword: (currentPassword: string, password: string) => Promise<void>;
  deleteAccount: (password: string) => Promise<void>;
}>();
const imported = ref<Journey | null>(null);
const importError = ref("");
const importMode = ref("merge");
const filename = ref("");
const legacy = ref<string | null>(null);
const currentPassword = ref("");
const newPassword = ref("");
const deletePassword = ref("");
const deleteConfirmation = ref("");
const showDelete = ref(false);
onMounted(() => {
  try {
    legacy.value = localStorage.getItem(legacyKey);
  } catch {
    /* La lettura locale è facoltativa. */
  }
});
function preview(raw: string, name: string) {
  imported.value = null;
  importError.value = "";
  filename.value = name;
  try {
    imported.value = parseImport(raw);
  } catch (reason) {
    importError.value = reason instanceof Error ? reason.message : "File non valido.";
  }
}
async function selectFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  if (file.size > maxFileSize) {
    imported.value = null;
    importError.value = "File troppo grande: massimo 2 MB.";
  } else {
    try {
      preview(await file.text(), file.name);
    } catch {
      importError.value = "Non riesco a leggere questo file.";
      imported.value = null;
    }
  }
  input.value = "";
}
async function applyImport() {
  if (!imported.value) return;
  importError.value = "";
  try {
    const next =
      importMode.value === "replace" ? imported.value : mergeJourney(props.journey, imported.value);
    await props.change(
      (value) => Object.assign(value, next),
      "Backup importato. Puoi annullare l’ultima modifica.",
    );
    imported.value = null;
  } catch (reason) {
    importError.value = reason instanceof Error ? reason.message : "Importazione non riuscita.";
  }
}
async function updatePassword() {
  await props.changePassword(currentPassword.value, newPassword.value);
  currentPassword.value = "";
  newPassword.value = "";
}
async function removeAccount() {
  if (deleteConfirmation.value !== "ELIMINA") return;
  await props.deleteAccount(deletePassword.value);
  deletePassword.value = "";
}
</script>

<template>
  <section class="page-heading">
    <p class="eyebrow">Controllo e libertà di scelta</p>
    <h1>Il tuo spazio.<br /><span>I tuoi dati.</span></h1>
    <p class="lead">
      Il percorso è associato a <strong>{{ username }}</strong
      >. Accedi allo stesso servizio con questo account per ritrovarlo su un altro dispositivo.
    </p>
  </section>
  <div class="board-grid settings-grid">
    <section class="card">
      <p class="eyebrow">Una pausa è parte del percorso</p>
      <h2>{{ journey.paused ? "Riprendi quando vuoi" : "Puoi fermarti qui" }}</h2>
      <p class="muted">
        La pausa ferma la registrazione di nuovi gesti e la conclusione degli obiettivi. Dati,
        interessi e possibilità di cambiare direzione restano disponibili.
      </p>
      <button
        :disabled="!canEdit"
        @click="
          change(
            (value) => {
              value.paused = !value.paused;
            },
            journey.paused
              ? 'Percorso ripreso, al tuo ritmo.'
              : 'Percorso in pausa. Nessun dato perso.',
          )
        "
      >
        {{ journey.paused ? "Riprendi il percorso" : "Metti in pausa" }}
      </button>
    </section>
    <section class="card">
      <p class="eyebrow">Una copia che controlli tu</p>
      <h2>Esporta il percorso</h2>
      <p class="muted">
        Il file JSON contiene passi, interessi, obiettivi e stato della pausa. Non contiene password
        né codici di accesso. Conserva la copia in un posto privato.
      </p>
      <button class="primary" @click="exportData">
        Scarica il mio percorso <span aria-hidden="true">↓</span>
      </button>
      <p class="hint">La copia include anche le modifiche ancora da sincronizzare.</p>
    </section>
    <section class="card import-card">
      <p class="eyebrow">Ritrova i tuoi passi</p>
      <h2>Ripristina da un file</h2>
      <p class="muted">
        Controlliamo il file prima di cambiare i dati. Puoi aggiungere il contenuto al percorso
        oppure sostituirlo.
      </p>
      <label class="file-label"
        >Scegli un backup Nexus<input
          type="file"
          accept=".json,application/json"
          :disabled="!canEdit"
          @change="selectFile"
      /></label>
      <div v-if="legacy" class="legacy-box">
        <p>
          Ci sono passi del vecchio prototipo in questo browser. Restano locali finché scegli di
          importarli in questo account.
        </p>
        <div class="button-row">
          <button :disabled="!canEdit" @click="preview(legacy!, 'Passi del vecchio prototipo')">
            Esamina i vecchi passi</button
          ><button @click="download('nexus-passi-originali.json', legacy!)">
            Scarica gli originali
          </button>
        </div>
      </div>
      <p v-if="importError" class="error-box" role="alert">{{ importError }}</p>
      <div v-if="imported" class="import-preview">
        <h3>{{ filename }}</h3>
        <p>
          {{ imported.steps.length }} passi · {{ imported.goals.length }} obiettivi ·
          {{ imported.interests.length }} interessi · {{ imported.paused ? "in pausa" : "attivo" }}
        </p>
        <fieldset>
          <legend>Come vuoi ripristinare?</legend>
          <label class="radio-label"
            ><input v-model="importMode" type="radio" value="merge" />Aggiungi al percorso, senza
            duplicare gli stessi elementi</label
          ><label class="radio-label"
            ><input v-model="importMode" type="radio" value="replace" />Sostituisci tutto il
            percorso con questo file</label
          >
        </fieldset>
        <p class="hint">
          Aggiungendo mantieni la pausa attuale. Sostituendo ripristini anche la pausa del file.
          Prima di sostituire puoi esportare la versione attuale.
        </p>
        <div class="button-row">
          <button class="primary" :disabled="!canEdit" @click="applyImport">
            Conferma ripristino</button
          ><button @click="imported = null">Annulla ripristino</button>
        </div>
      </div>
    </section>
    <section class="card">
      <p class="eyebrow">Accesso personale</p>
      <h2>Cambia password</h2>
      <p class="muted">
        Gli altri dispositivi dovranno accedere di nuovo. Riceverai anche un nuovo codice di
        recupero.
      </p>
      <form class="form-stack" @submit.prevent="updatePassword">
        <label
          >Password attuale<input
            v-model="currentPassword"
            required
            type="password"
            maxlength="128"
            autocomplete="current-password" /></label
        ><label
          >Nuova password<input
            v-model="newPassword"
            required
            type="password"
            minlength="12"
            maxlength="128"
            autocomplete="new-password" /></label
        ><button type="submit" :disabled="busy">Aggiorna password</button>
      </form>
    </section>
    <section class="card privacy-card">
      <p class="eyebrow">Dove vive il tuo percorso</p>
      <h2>Account privato, dati separati</h2>
      <p>
        Il server conserva il percorso e lo rende disponibile ai dispositivi su cui accedi. Una
        sessione dura fino a sette giorni. Le modifiche non ancora inviate hanno una copia locale
        nel browser, quando disponibile.
      </p>
      <p>
        La password è salvata come impronta crittografica. I dati del percorso sono leggibili
        dall’amministratore del server e dai suoi backup: non c’è cifratura da dispositivo a
        dispositivo.
      </p>
      <p>
        I dati personali non vengono scritti sulla blockchain. Questo spazio non attiva operazioni
        di GIANO.
      </p>
    </section>
    <section class="card danger-card">
      <p class="eyebrow">Una scelta definitiva</p>
      <h2>Elimina account</h2>
      <p class="muted">
        Elimina l’account e il percorso dal servizio, revocando tutti gli accessi. Le copie
        esportate, le bozze sugli altri dispositivi e i backup del server vanno gestiti
        separatamente.
      </p>
      <button v-if="!showDelete" class="danger" :disabled="busy" @click="showDelete = true">
        Voglio eliminare l’account
      </button>
      <form v-else class="form-stack" @submit.prevent="removeAccount">
        <label
          >Password per confermare<input
            v-model="deletePassword"
            required
            type="password"
            maxlength="128"
            autocomplete="current-password" /></label
        ><label
          >Scrivi ELIMINA<input
            v-model="deleteConfirmation"
            required
            pattern="ELIMINA"
            autocomplete="off"
        /></label>
        <div class="button-row">
          <button class="danger" type="submit" :disabled="busy || deleteConfirmation !== 'ELIMINA'">
            Elimina definitivamente</button
          ><button
            type="button"
            @click="
              showDelete = false;
              deletePassword = '';
              deleteConfirmation = '';
            "
          >
            Annulla
          </button>
        </div>
      </form>
    </section>
  </div>
</template>
