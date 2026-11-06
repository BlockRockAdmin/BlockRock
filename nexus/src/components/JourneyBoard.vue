<script setup lang="ts">
import { computed, ref } from "vue";
import { makeId } from "../domain/journey";
import type { Goal, Journey } from "../domain/journey";
const props = defineProps<{
  journey: Journey;
  canEdit: boolean;
  page: "today" | "personal" | "stability";
  change: (update: (value: Journey) => void, announcement: string) => Promise<void>;
}>();
const suggestions = [
  "Ho cucinato qualcosa",
  "Sono uscito",
  "Ho sentito una persona",
  "Ho rimesso in ordine",
];
const ownAction = ref("");
const search = ref("");
const visibleCount = ref(20);
const editingStep = ref("");
const stepText = ref("");
const goalTitle = ref("");
const nextStep = ref("");
const editingGoal = ref("");
const newInterest = ref("");
const area = computed(() => (props.page === "stability" ? "stability" : "personal"));
const goals = computed(() => props.journey.goals.filter((goal) => goal.area === area.value));
const history = computed(() =>
  [...props.journey.steps]
    .filter((step) =>
      step.action.toLocaleLowerCase("it").includes(search.value.toLocaleLowerCase("it")),
    )
    .sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt)),
);
const todayCount = computed(
  () =>
    props.journey.steps.filter(
      (step) => new Date(step.createdAt).toLocaleDateString() === new Date().toLocaleDateString(),
    ).length,
);
const formatDate = (date: string) =>
  new Intl.DateTimeFormat("it-IT", { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(date),
  );
async function record(action: string) {
  if (!action.trim() || props.journey.paused) return;
  await props.change((value) => {
    value.steps.push({ id: makeId(), action: action.trim(), createdAt: new Date().toISOString() });
  }, "Passo registrato. +1, al tuo ritmo.");
  ownAction.value = "";
}
async function saveStep() {
  await props.change((value) => {
    const entry = value.steps.find((step) => step.id === editingStep.value);
    if (entry) entry.action = stepText.value.trim();
  }, "Passo aggiornato.");
  editingStep.value = "";
}
function editGoal(goal: Goal) {
  editingGoal.value = goal.id;
  goalTitle.value = goal.title;
  nextStep.value = goal.nextStep;
}
function cancelGoal() {
  editingGoal.value = "";
  goalTitle.value = "";
  nextStep.value = "";
}
async function saveGoal() {
  await props.change(
    (value) => {
      const existing = value.goals.find((goal) => goal.id === editingGoal.value);
      if (existing) {
        existing.title = goalTitle.value.trim();
        existing.nextStep = nextStep.value.trim();
      } else
        value.goals.push({
          id: makeId(),
          title: goalTitle.value.trim(),
          nextStep: nextStep.value.trim(),
          area: area.value,
          done: false,
        });
    },
    editingGoal.value ? "Obiettivo aggiornato." : "Una nuova direzione, scelta da te.",
  );
  cancelGoal();
}
async function addInterest() {
  await props.change((value) => {
    value.interests.push(newInterest.value.trim());
  }, "Interesse aggiunto.");
  newInterest.value = "";
}
</script>

<template>
  <template v-if="page === 'today'">
    <section class="page-heading">
      <p class="eyebrow">Il presente, una cosa alla volta</p>
      <h1>Anche una cosa<br /><span>piccola conta.</span></h1>
      <p class="lead">
        Scegli un gesto che hai fatto per te. Non devi spiegare il perché e non c’è un percorso
        giusto da seguire.
      </p>
    </section>
    <div class="stats-row">
      <div>
        <strong>{{ todayCount }}</strong
        ><span>passi segnati oggi</span>
      </div>
      <div>
        <strong>{{ journey.steps.length }}</strong
        ><span>gesti nel tuo percorso</span>
      </div>
      <div>
        <strong>{{ journey.paused ? "In pausa" : "Al tuo ritmo" }}</strong
        ><span>nessuna scadenza, nessuna penalità</span>
      </div>
    </div>
    <div class="board-grid">
      <section class="card">
        <div class="card-heading">
          <div>
            <p class="eyebrow">Un gesto per te</p>
            <h2>Che cosa hai fatto?</h2>
          </div>
          <span class="plus-badge" aria-hidden="true">+1</span>
        </div>
        <div class="suggestions">
          <button
            v-for="action in suggestions"
            :key="action"
            :disabled="!canEdit || journey.paused"
            @click="record(action)"
          >
            <span aria-hidden="true">＋</span>{{ action }}
          </button>
        </div>
        <form class="form-stack" @submit.prevent="record(ownAction)">
          <label
            >Oppure un gesto tuo<input
              v-model="ownAction"
              maxlength="120"
              required
              placeholder="Una cosa piccola, come la definisci tu"
              :disabled="!canEdit || journey.paused" /></label
          ><button
            type="submit"
            class="primary"
            :disabled="!canEdit || journey.paused || !ownAction.trim()"
          >
            Segna +1 <span aria-hidden="true">↗</span>
          </button>
        </form>
        <p class="gentle-note">
          Il +1 racconta un gesto. Non è un voto e non serve a tenere una serie di giorni perfetti.
        </p>
      </section>
      <section class="card history-card">
        <div class="card-heading">
          <div>
            <p class="eyebrow">Il tuo percorso</p>
            <h2>I passi che hai scelto</h2>
          </div>
          <span class="count">{{ journey.steps.length }}</span>
        </div>
        <label v-if="journey.steps.length" class="search-label"
          >Cerca nel registro<input
            v-model="search"
            type="search"
            placeholder="Cerca un gesto"
            @input="visibleCount = 20"
        /></label>
        <div v-if="!journey.steps.length" class="empty">
          <span aria-hidden="true">◌</span>
          <h3>Qui c’è spazio per iniziare.</h3>
          <p>I gesti che scegli di segnare appariranno qui. Quando ti va.</p>
        </div>
        <p v-else-if="!history.length" class="muted">Nessun gesto corrisponde alla ricerca.</p>
        <ul v-else class="entries">
          <li v-for="entry in history.slice(0, visibleCount)" :key="entry.id">
            <span class="entry-mark" aria-hidden="true">+1</span>
            <div class="entry-body">
              <form v-if="editingStep === entry.id" class="form-stack" @submit.prevent="saveStep">
                <label>Modifica gesto<input v-model="stepText" required maxlength="120" /></label>
                <div class="button-row">
                  <button type="submit" :disabled="!canEdit">Salva gesto</button
                  ><button type="button" @click="editingStep = ''">Annulla</button>
                </div>
              </form>
              <template v-else
                ><strong>{{ entry.action }}</strong
                ><time :datetime="entry.createdAt">{{ formatDate(entry.createdAt) }}</time>
                <div class="entry-actions">
                  <button
                    :disabled="!canEdit"
                    :aria-label="`Modifica: ${entry.action}`"
                    @click="
                      editingStep = entry.id;
                      stepText = entry.action;
                    "
                  >
                    Modifica</button
                  ><button
                    :disabled="!canEdit"
                    :aria-label="`Rimuovi: ${entry.action}`"
                    @click="
                      change((value) => {
                        value.steps = value.steps.filter((step) => step.id !== entry.id);
                      }, 'Passo rimosso. Puoi annullare la modifica.')
                    "
                  >
                    Rimuovi
                  </button>
                </div></template
              >
            </div>
          </li>
        </ul>
        <button
          v-if="history.length > visibleCount"
          class="text-button"
          @click="visibleCount += 20"
        >
          Mostra altri 20
        </button>
      </section>
    </div>
  </template>
  <template v-else>
    <section class="page-heading">
      <p class="eyebrow">
        {{ page === "stability" ? "Le risorse, con chiarezza" : "Segui quello che senti tuo" }}
      </p>
      <h1>
        {{ page === "stability" ? "Costruisci la tua" : "Ritrova la tua" }}<br /><span>{{
          page === "stability" ? "stabilità." : "direzione."
        }}</span>
      </h1>
      <p class="lead">
        {{
          page === "stability"
            ? "Scegli un obiettivo economico concreto e un prossimo passo sostenibile: capire le spese, costruire una riserva, riprendere un’abitudine."
            : "Interessi da riprendere, capacità da esplorare, obiettivi da cambiare. Parti da quello che conta per te adesso."
        }}
      </p>
    </section>
    <p v-if="page === 'stability'" class="info-strip">
      Questa è la tua pianificazione personale. Non mostra un saldo bancario e non invia ordini.
      Passi e obiettivi non cambiano il rischio o il capitale di GIANO.
    </p>
    <section v-if="page === 'personal'" class="card interests-card">
      <h2>Cosa ti interessa?</h2>
      <p class="muted">Non serve che diventi un obiettivo. Può essere soltanto una curiosità.</p>
      <div class="chips">
        <span v-for="interest in journey.interests" :key="interest"
          >{{ interest
          }}<button
            :disabled="!canEdit"
            :aria-label="`Rimuovi interesse: ${interest}`"
            @click="
              change((value) => {
                value.interests = value.interests.filter((item) => item !== interest);
              }, 'Interesse rimosso.')
            "
          >
            ×
          </button></span
        >
      </div>
      <form class="inline-form" @submit.prevent="addInterest">
        <label class="grow"
          >Un tuo interesse<input
            v-model="newInterest"
            required
            maxlength="60"
            placeholder="Musica, cucina, una lingua…"
            :disabled="!canEdit" /></label
        ><button type="submit" :disabled="!canEdit || !newInterest.trim()">
          Aggiungi interesse
        </button>
      </form>
    </section>
    <div class="board-grid goals-grid">
      <section class="card">
        <p class="eyebrow">Una direzione possibile</p>
        <h2>{{ editingGoal ? "Cambia il tuo obiettivo" : "Scegli un obiettivo" }}</h2>
        <form class="form-stack" @submit.prevent="saveGoal">
          <label
            >Cosa vuoi provare a fare?<input
              v-model="goalTitle"
              required
              maxlength="160"
              :disabled="!canEdit"
              :placeholder="
                page === 'stability'
                  ? 'Mettere da parte una piccola riserva'
                  : 'Riprendere a suonare'
              " /></label
          ><label
            >Qual è il prossimo piccolo passo?<textarea
              v-model="nextStep"
              maxlength="240"
              rows="3"
              :disabled="!canEdit"
              :placeholder="
                page === 'stability'
                  ? 'Riguardare insieme le spese del mese'
                  : 'Trovare dieci minuti per riprendere lo strumento'
              "
            ></textarea
            ><span class="hint">Facoltativo. Puoi cambiarlo quando vuoi.</span></label
          >
          <div class="button-row">
            <button type="submit" class="primary" :disabled="!canEdit || !goalTitle.trim()">
              {{ editingGoal ? "Salva obiettivo" : "Aggiungi obiettivo" }}</button
            ><button v-if="editingGoal" type="button" @click="cancelGoal">Annulla</button>
          </div>
        </form>
      </section>
      <section class="goal-list" aria-label="I tuoi obiettivi">
        <div v-if="!goals.length" class="card empty">
          <span aria-hidden="true">↗</span>
          <h2>Nessun traguardo imposto.</h2>
          <p>Il primo obiettivo può essere piccolo. O può aspettare.</p>
        </div>
        <article
          v-for="goal in goals"
          :key="goal.id"
          class="card goal-card"
          :class="{ completed: goal.done }"
        >
          <p class="eyebrow">{{ goal.done ? "Concluso, a modo tuo" : "Una direzione aperta" }}</p>
          <h3>{{ goal.title }}</h3>
          <p v-if="goal.nextStep">{{ goal.nextStep }}</p>
          <p v-else class="muted">Il prossimo passo lo sceglierai tu.</p>
          <div class="button-row">
            <button
              :disabled="!canEdit || (journey.paused && !goal.done)"
              @click="
                change(
                  (value) => {
                    const item = value.goals.find((item) => item.id === goal.id);
                    if (item) item.done = !item.done;
                  },
                  goal.done ? 'Obiettivo riaperto.' : 'Obiettivo concluso, a modo tuo.',
                )
              "
            >
              {{ goal.done ? "Riapri" : "Segna come concluso" }}</button
            ><button :disabled="!canEdit" @click="editGoal(goal)">Modifica</button
            ><button
              :disabled="!canEdit"
              @click="
                change((value) => {
                  value.goals = value.goals.filter((item) => item.id !== goal.id);
                }, 'Obiettivo rimosso. Puoi annullare la modifica.');
                cancelGoal();
              "
            >
              Rimuovi
            </button>
          </div>
        </article>
      </section>
    </div>
  </template>
</template>
