<script setup lang="ts">
import { reactive, ref, watch } from "vue";
import AccountGate from "./AccountGate.vue";
import DataPanel from "./DataPanel.vue";
import JourneyBoard from "./JourneyBoard.vue";
import PersonalGateway from "./PersonalGateway.vue";
import { useJourney } from "../composables/useJourney";
import { download } from "../domain/journey";
const app = reactive(useJourney());
const page = ref<"today" | "personal" | "stability" | "gateway" | "data">("today");
const confirmLocal = ref(false);
watch(
  () => app.user?.id,
  () => {
    page.value = "today";
  },
);
const date = new Intl.DateTimeFormat("it-IT", {
  weekday: "long",
  day: "numeric",
  month: "long",
}).format(new Date());
const formatTime = (value: string) =>
  new Intl.DateTimeFormat("it-IT", { dateStyle: "short", timeStyle: "short" }).format(
    new Date(value),
  );
</script>

<template>
  <div class="nexus-shell">
    <a href="#main-content" class="skip-link">Vai al contenuto</a>
    <header class="topbar">
      <a
        class="brand"
        :href="app.pending || app.conflict ? undefined : '/'"
        aria-label="Nexus, pagina iniziale"
        ><span class="brand-mark" aria-hidden="true">N</span
        ><span>NEXUS<small>by BlockRock</small></span></a
      ><span class="topbar-note">Il tuo spazio, al tuo ritmo.</span>
      <div v-if="app.user" class="account-menu">
        <span>{{ app.user.displayName }}</span
        ><button :disabled="app.busy || !!app.recoveryCode" @click="app.logout">Esci</button>
      </div>
      <span v-else class="private-label">Spazio privato</span>
    </header>
    <main id="main-content" class="content">
      <div v-if="app.loading" class="card loading" role="status">Sto aprendo il tuo spazio…</div>
      <template v-else>
        <div v-if="app.error" class="error-box global-message" role="alert">
          {{ app.error
          }}<button v-if="!app.user" :disabled="app.busy" @click="app.boot">
            Riprova connessione
          </button>
        </div>
        <div v-if="app.notice" class="notice-box global-message" role="status">
          {{ app.notice
          }}<button v-if="app.undoState && app.canEdit" @click="app.undo">
            Annulla ultima modifica
          </button>
        </div>
        <div v-if="app.sessionExpired && (app.pending || app.conflict)" class="info-strip">
          Le modifiche di questo dispositivo sono conservate in attesa dell’accesso.<button
            @click="app.exportData"
          >
            Esporta le modifiche in sospeso
          </button>
        </div>
        <AccountGate
          v-if="!app.user || app.sessionExpired"
          :busy="app.busy"
          :registration-enabled="app.registrationEnabled"
          :renew-username="app.sessionExpired ? app.user?.username : undefined"
          @authenticate="app.authenticate"
        />
        <template v-else>
          <section
            v-if="app.recoveryCode"
            class="recovery-card card"
            aria-labelledby="recovery-title"
          >
            <p class="eyebrow">Un codice solo tuo</p>
            <h2 id="recovery-title">Conserva la chiave del tuo spazio.</h2>
            <p>
              Se dimentichi la password, questo codice ti permette di recuperare l’account. È
              mostrato solo ora: salvalo in un posto privato. Non condividerlo.
            </p>
            <code>{{ app.recoveryCode }}</code>
            <div class="button-row">
              <button
                class="primary"
                @click="
                  download(
                    'nexus-codice-recupero.json',
                    JSON.stringify(
                      { username: app.user!.username, recoveryCode: app.recoveryCode },
                      null,
                      2,
                    ),
                  )
                "
              >
                Scarica il codice di recupero</button
              ><button @click="app.recoveryCode = ''">L’ho conservato, continua</button>
            </div>
          </section>
          <div class="workspace-bar">
            <nav class="page-nav" aria-label="Il tuo spazio">
              <button :aria-current="page === 'today' ? 'page' : undefined" @click="page = 'today'">
                Oggi</button
              ><button
                :aria-current="page === 'personal' ? 'page' : undefined"
                @click="page = 'personal'"
              >
                La mia direzione</button
              ><button
                :aria-current="page === 'stability' ? 'page' : undefined"
                @click="page = 'stability'"
              >
                Stabilità</button
              ><button :aria-current="page === 'gateway' ? 'page' : undefined" @click="page = 'gateway'">
                Il mio quadro</button
              ><button :aria-current="page === 'data' ? 'page' : undefined" @click="page = 'data'">
                Dati e account
              </button>
            </nav>
            <span class="today-date">{{ date }}</span>
          </div>
          <div class="sync-bar" role="status">
            <span
              class="sync-dot"
              :class="{ pending: app.pending || app.conflict }"
              aria-hidden="true"
            ></span
            ><strong>{{ app.syncLabel }}</strong
            ><span v-if="app.lastSynced" class="sync-time"
              >Ultimo salvataggio {{ formatTime(app.lastSynced) }}</span
            ><button :disabled="app.busy || app.conflict" @click="app.sync">
              {{ app.busy ? "Attendi…" : "Sincronizza ora" }}
            </button>
          </div>
          <section v-if="app.conflict" class="conflict-card card" aria-labelledby="conflict-title">
            <h2 id="conflict-title">Conserviamo la versione giusta.</h2>
            <p>
              Le modifiche di questo dispositivo sono visibili qui. Il server o un’altra scheda
              hanno una versione diversa. Esporta prima le copie che vuoi conservare, poi scegli
              quale usare.
            </p>
            <div class="button-row">
              <button @click="app.exportData">Esporta questa versione</button
              ><button @click="app.exportDraft">Scarica copia locale originale</button
              ><button :disabled="app.busy" @click="app.resolveConflict(false)">
                Carica server e scarta le modifiche locali
              </button>
            </div>
            <label class="radio-label"
              ><input v-model="confirmLocal" type="checkbox" />Voglio sostituire il percorso sul
              server con quello visibile su questo dispositivo.</label
            ><button
              :disabled="!confirmLocal || app.busy"
              @click="
                app.resolveConflict(true);
                confirmLocal = false;
              "
            >
              Sostituisci la versione sul server
            </button>
          </section>
          <div v-if="app.journey.paused" class="pause-banner" role="status">
            <span>◷ Il percorso è in pausa. I tuoi dati restano qui.</span
            ><button
              :disabled="!app.canEdit"
              @click="
                app.change((value) => {
                  value.paused = false;
                }, 'Percorso ripreso, al tuo ritmo.')
              "
            >
              Riprendi il percorso
            </button>
          </div>
          <PersonalGateway v-if="page === 'gateway'" />
          <DataPanel
            v-else-if="page === 'data'"
            :journey="app.journey"
            :username="app.user.username"
            :can-edit="app.canEdit"
            :busy="app.busy"
            :change="app.change"
            :export-data="app.exportData"
            :change-password="app.changePassword"
            :delete-account="app.deleteAccount"
          />
          <JourneyBoard
            v-else
            :key="page"
            :journey="app.journey"
            :page="page"
            :can-edit="app.canEdit"
            :change="app.change"
          />
        </template>
      </template>
    </main>
    <footer class="page-footer">
      <span>Il percorso è tuo. Puoi cambiarlo in ogni momento.</span
      ><span>Nexus · BlockRock <span aria-hidden="true">↗</span></span>
    </footer>
  </div>
</template>
