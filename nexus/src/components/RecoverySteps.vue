<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

type RecoveryStep = {
  id: string
  action: string
  createdAt: string
}

const storageKey = 'blockrock.recovery-steps.v1'
const suggestedActions = [
  'Ho cucinato qualcosa',
  'Sono uscito',
  'Ho sentito una persona',
  'Ho rimesso in ordine',
]

const steps = ref<RecoveryStep[]>([])
const ownAction = ref('')
const announcement = ref('')
const storageMessage = ref('')
const totalSteps = computed(() => steps.value.length)

const newestFirst = computed(() =>
  [...steps.value].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
)

onMounted(() => {
  try {
    const saved = window.localStorage.getItem(storageKey)
    if (!saved) return

    const parsed: unknown = JSON.parse(saved)
    if (!Array.isArray(parsed)) return

    steps.value = parsed.filter(
      (item): item is RecoveryStep =>
        typeof item?.id === 'string' &&
        typeof item?.action === 'string' &&
        typeof item?.createdAt === 'string',
    )
  } catch {
    storageMessage.value = 'Non riesco a leggere i passi salvati su questo dispositivo.'
  }
})

function saveSteps() {
  try {
    window.localStorage.setItem(storageKey, JSON.stringify(steps.value))
    storageMessage.value = ''
  } catch {
    storageMessage.value = 'Salvataggio non disponibile: il passo resta visibile solo finché la pagina è aperta.'
  }
}

function recordStep(action: string) {
  const cleanAction = action.trim()
  if (!cleanAction) return

  steps.value.unshift({
    id: window.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`,
    action: cleanAction,
    createdAt: new Date().toISOString(),
  })
  ownAction.value = ''
  saveSteps()
  announcement.value = `Passo registrato: ${cleanAction}. +1.`
}

function removeStep(id: string) {
  steps.value = steps.value.filter((step) => step.id !== id)
  saveSteps()
  announcement.value = 'Passo rimosso.'
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat('it-IT', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value))
}
</script>

<template>
  <div class="recovery-page">
    <header class="topbar">
      <a class="brand" href="/" aria-label="BlockRock, pagina iniziale">
        <span class="brand-mark" aria-hidden="true">B</span>
        <span>BlockRock</span>
      </a>
      <span class="topbar-note">Il tuo spazio, al tuo ritmo</span>
    </header>

    <main class="content">
      <section class="intro" aria-labelledby="page-title">
        <p class="eyebrow">Un passo alla volta</p>
        <h1 id="page-title">Anche una cosa piccola conta.</h1>
        <p class="intro-copy">
          Scegli un gesto che hai fatto per te. Non devi spiegare il perché e non c'è
          un percorso giusto da seguire.
        </p>
      </section>

      <div class="workspace">
        <section class="step-card" aria-labelledby="step-title">
          <div class="card-heading">
            <div>
              <p class="eyebrow">Oggi</p>
              <h2 id="step-title">Che cosa hai fatto?</h2>
            </div>
            <span class="plus-one" aria-label="Un passo vale più uno">+1</span>
          </div>

          <div class="suggestions" aria-label="Gesti suggeriti">
            <button
              v-for="action in suggestedActions"
              :key="action"
              class="action-button"
              type="button"
              @click="recordStep(action)"
            >
              <span class="action-plus" aria-hidden="true">+</span>
              {{ action }}
            </button>
          </div>

          <form class="custom-action" @submit.prevent="recordStep(ownAction)">
            <label for="own-action">Oppure scegli un gesto tuo</label>
            <div class="custom-row">
              <input
                id="own-action"
                v-model="ownAction"
                maxlength="80"
                autocomplete="off"
                placeholder="Una cosa piccola, come la definisci tu"
              />
              <button class="submit-button" type="submit" :disabled="!ownAction.trim()">
                Segna +1
              </button>
            </div>
          </form>

          <p class="gentle-note">Il +1 è un passo, non un voto. Puoi fermarti o riprendere quando vuoi.</p>
          <p v-if="storageMessage" class="storage-message" role="status">{{ storageMessage }}</p>
          <p class="privacy-note">I dati restano in questo browser e non si sincronizzano. Evita dettagli intimi.</p>
          <p class="visually-hidden" aria-live="polite">{{ announcement }}</p>
        </section>

        <aside class="history-card" aria-labelledby="history-title">
          <div class="history-heading">
            <div>
              <p class="eyebrow">Il tuo percorso</p>
              <h2 id="history-title">Passi registrati</h2>
            </div>
            <span class="step-count" aria-label="Totale passi registrati">{{ totalSteps }}</span>
          </div>

          <p v-if="newestFirst.length === 0" class="empty-state">
            Quando ti va, qui troverai i gesti che hai scelto di segnare.
          </p>

          <ul v-else class="step-list">
            <li v-for="step in newestFirst" :key="step.id" class="step-entry">
              <span class="entry-dot" aria-hidden="true">+1</span>
              <div class="entry-copy">
                <strong>{{ step.action }}</strong>
                <time :datetime="step.createdAt">{{ formatDate(step.createdAt) }}</time>
              </div>
              <button
                class="remove-button"
                type="button"
                :aria-label="`Rimuovi: ${step.action}`"
                @click="removeStep(step.id)"
              >
                Rimuovi
              </button>
            </li>
          </ul>
        </aside>
      </div>

      <footer class="page-footer">
        <span>Il percorso è tuo. Puoi cambiarlo in ogni momento.</span>
        <span>Prototipo locale · nessuna diagnosi · nessuna operazione finanziaria</span>
      </footer>
    </main>
  </div>
</template>

<style scoped>
:global(*) { box-sizing: border-box; }
:global(body) {
  display: block;
  min-width: 320px;
  min-height: 100vh;
  margin: 0;
  background: #f4f6f3;
  color: #1e302e;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
:global(#app) { width: 100%; max-width: none; margin: 0; padding: 0; font-weight: 400; }

.recovery-page { min-height: 100vh; background: #f4f6f3; }
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  max-width: 1160px;
  margin: 0 auto;
  padding: 25px 32px;
}
.brand { display: inline-flex; align-items: center; gap: 10px; color: #203632; font-size: 17px; font-weight: 750; text-decoration: none; }
.brand-mark { display: grid; width: 34px; height: 34px; place-items: center; border-radius: 11px; background: #24675b; color: #fff; font-family: Georgia, serif; font-size: 21px; }
.topbar-note { color: #667772; font-size: 13px; }
.content { max-width: 1040px; margin: 0 auto; padding: 55px 32px 32px; }
.intro { max-width: 620px; margin: 0 0 34px; }
.eyebrow { margin: 0 0 10px; color: #3a766b; font-size: 11px; font-weight: 750; letter-spacing: .12em; text-transform: uppercase; }
h1, h2, p { margin-top: 0; }
h1 { max-width: 580px; margin-bottom: 14px; color: #203632; font-family: Georgia, "Times New Roman", serif; font-size: clamp(36px, 5vw, 54px); font-weight: 500; letter-spacing: -.035em; line-height: 1.06; }
.intro-copy { max-width: 540px; margin: 0; color: #5d6d68; font-size: 16px; line-height: 1.7; }
.workspace { display: grid; grid-template-columns: minmax(0, 1.15fr) minmax(280px, .85fr); gap: 18px; align-items: start; }
.step-card, .history-card { border: 1px solid #e1e8e3; border-radius: 20px; background: #fff; box-shadow: 0 12px 34px rgb(33 62 54 / 5%); }
.step-card { padding: 28px; }
.history-card { padding: 25px; }
.card-heading, .history-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
h2 { margin: 0; color: #223a35; font-size: 20px; font-weight: 680; letter-spacing: -.02em; }
.plus-one { display: grid; width: 46px; height: 46px; place-items: center; border: 1px solid #d8e9e1; border-radius: 15px; background: #eff7f2; color: #327265; font-size: 18px; font-weight: 750; }
.suggestions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin: 25px 0 24px; }
.action-button { display: flex; min-height: 54px; align-items: center; gap: 10px; padding: 10px 12px; border: 1px solid #e0e8e2; border-radius: 12px; background: #fbfcfa; color: #2d4540; cursor: pointer; font: inherit; font-size: 13px; text-align: left; transition: border-color .15s ease, background .15s ease, transform .15s ease; }
.action-button:hover { transform: translateY(-1px); border-color: #9dc5b7; background: #f4faf6; }
.action-button:focus-visible, .submit-button:focus-visible, .remove-button:focus-visible, input:focus-visible { outline: 3px solid #83b8a8; outline-offset: 2px; }
.action-plus { display: grid; width: 25px; height: 25px; flex: 0 0 25px; place-items: center; border-radius: 8px; background: #eaf3ed; color: #327265; font-size: 18px; }
.custom-action label { display: block; margin-bottom: 8px; color: #53655f; font-size: 12px; font-weight: 650; }
.custom-row { display: flex; gap: 8px; }
.custom-row input { width: 100%; min-width: 0; min-height: 44px; padding: 0 12px; border: 1px solid #dce5df; border-radius: 10px; background: #fff; color: #243a34; font: inherit; font-size: 13px; }
.custom-row input::placeholder { color: #9aa8a1; }
.submit-button { flex: 0 0 auto; padding: 0 15px; border: 0; border-radius: 10px; background: #24675b; color: #fff; cursor: pointer; font: inherit; font-size: 13px; font-weight: 700; }
.submit-button:hover:not(:disabled) { background: #1c544a; }
.submit-button:disabled { cursor: not-allowed; opacity: .45; }
.gentle-note { margin: 18px 0 0; color: #596d65; font-size: 12px; line-height: 1.6; }
.privacy-note, .storage-message { margin: 11px 0 0; color: #82918a; font-size: 11px; line-height: 1.5; }
.storage-message { color: #9a5a25; }
.step-count { display: grid; min-width: 38px; height: 38px; place-items: center; padding: 0 9px; border-radius: 13px; background: #f0f5f0; color: #327265; font-size: 16px; font-weight: 750; }
.empty-state { margin: 24px 0 4px; color: #75847e; font-size: 13px; line-height: 1.65; }
.step-list { display: grid; gap: 0; margin: 20px 0 0; padding: 0; list-style: none; }
.step-entry { display: flex; align-items: center; gap: 11px; padding: 13px 0; border-top: 1px solid #edf1ed; }
.entry-dot { display: grid; width: 34px; height: 34px; flex: 0 0 34px; place-items: center; border-radius: 11px; background: #f0f7f2; color: #327265; font-size: 11px; font-weight: 750; }
.entry-copy { display: grid; min-width: 0; gap: 4px; flex: 1; }
.entry-copy strong { overflow: hidden; color: #304741; font-size: 12px; font-weight: 650; text-overflow: ellipsis; white-space: nowrap; }
.entry-copy time { color: #91a098; font-size: 10px; }
.remove-button { padding: 6px 0 6px 6px; border: 0; background: transparent; color: #83918b; cursor: pointer; font: inherit; font-size: 10px; }
.remove-button:hover { color: #8b4c47; text-decoration: underline; }
.page-footer { display: flex; justify-content: space-between; gap: 16px; margin-top: 25px; color: #8a9891; font-size: 10px; }
.visually-hidden { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; clip-path: inset(50%); }

@media (max-width: 760px) {
  .topbar { padding: 18px 20px; }
  .content { padding: 42px 20px 24px; }
  .workspace { grid-template-columns: 1fr; }
  .page-footer { flex-direction: column; }
}
@media (max-width: 480px) {
  .topbar-note { font-size: 11px; }
  .step-card, .history-card { padding: 20px; }
  .suggestions { grid-template-columns: 1fr; }
  .custom-row { align-items: stretch; flex-direction: column; }
  .submit-button { min-height: 43px; }
}
</style>
