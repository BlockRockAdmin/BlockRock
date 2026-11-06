<script setup lang="ts">
import { computed, reactive, ref } from "vue";

type Area = "amore" | "capacita" | "mondo" | "risorse";
type Feeling = "non_definito" | "equilibrio" | "pressione";
const areas: { id: Area; name: string; hint: string; color: string; cx: number; cy: number }[] = [
  { id: "amore", name: "Ciò che ami", hint: "Interessi, piaceri e ciò che ti fa sentire vivo.", color: "#ff486a", cx: 170, cy: 116 },
  { id: "capacita", name: "Le tue capacità", hint: "Ciò che sai fare e ciò che desideri imparare.", color: "#62f3a6", cx: 224, cy: 169 },
  { id: "mondo", name: "Il mondo intorno", hint: "Relazioni, bisogni e contributo agli altri.", color: "#2ac984", cx: 170, cy: 222 },
  { id: "risorse", name: "Le tue risorse", hint: "Sostentamento, lavoro e spazio economico.", color: "#d63357", cx: 116, cy: 169 },
];
const selected = ref<Area>("amore");
const values = reactive<Record<Area, Feeling>>({ amore: "non_definito", capacita: "non_definito", mondo: "non_definito", risorse: "non_definito" });
const labels: Record<Feeling, string> = { non_definito: "Da esplorare", equilibrio: "Trova spazio", pressione: "Sotto pressione" };
const current = computed(() => areas.find((area) => area.id === selected.value)!);
</script>

<template>
  <section class="prism" aria-labelledby="prism-title">
    <div class="prism-intro">
      <div><p class="prism-kicker"><span></span> UN QUADRO CHE PARTE DA TE</p><h1 id="prism-title">Il tuo <em>prisma</em></h1>
      <p>La vita ha più dimensioni. Guardarle insieme può aiutarti a vedere dove trovi spazio e dove senti una resistenza.</p></div>
      <span class="prism-tag">ISPIRATO ALL’IKIGAI</span>
    </div>
    <div class="prism-layout">
      <div class="prism-visual">
        <p class="prism-visual-label">LE DIMENSIONI DEL TUO PERCORSO</p>
        <svg viewBox="0 0 340 340" role="img" aria-label="Quattro cerchi sovrapposti: ciò che ami, capacità, mondo intorno e risorse. Scegli un'area nelle schede accanto.">
          <circle cx="170" cy="169" r="153" fill="none" stroke="#38383d" stroke-dasharray="2 7" />
          <circle v-for="area in areas" :key="area.id" :cx="area.cx" :cy="area.cy" r="82" :fill="area.color" :fill-opacity="selected === area.id ? .27 : .12" :stroke="values[area.id] === 'pressione' ? '#ff6683' : values[area.id] === 'equilibrio' ? '#50efa0' : area.color" :stroke-dasharray="values[area.id] === 'pressione' ? '5 4' : undefined" :stroke-opacity="selected === area.id || values[area.id] !== 'non_definito' ? .95 : .5" :stroke-width="values[area.id] === 'non_definito' ? 1.3 : 2.5" />
          <circle cx="170" cy="169" r="31" fill="#101112" stroke="#b1d1dc" stroke-opacity=".65" />
          <path d="M170 151v36M152 169h36M158 157l24 24M182 157l-24 24" stroke="#c3dce0" stroke-width="1" opacity=".65" />
          <circle cx="170" cy="169" r="4" fill="#e1f1ec" />
          <text x="170" y="62" text-anchor="middle">AMORE</text><text x="284" y="172" text-anchor="middle">CAPACITÀ</text><text x="170" y="284" text-anchor="middle">MONDO</text><text x="56" y="172" text-anchor="middle">RISORSE</text>
        </svg>
        <p class="prism-visual-foot">Al centro ci sei tu, non un punteggio.</p>
        <p class="prism-key">Linea tratteggiata: sotto pressione · Linea continua marcata: trova spazio</p>
      </div>
      <div class="prism-content">
        <div class="prism-content-heading"><div><p class="prism-small">ESPLORA IL TUO EQUILIBRIO</p><h2>Da dove vuoi guardare?</h2></div><span>01 — 04</span></div>
        <p class="prism-description">Scegli un’area e descrivi come la senti oggi. Puoi dare spazio a una parte anche quando un’altra è sotto pressione.</p>
        <div class="prism-areas" role="group" aria-label="Aree del prisma">
          <button v-for="(area, index) in areas" :key="area.id" type="button" class="prism-area" :class="{ active: selected === area.id }" :aria-pressed="selected === area.id" @click="selected = area.id">
            <span class="area-icon" :style="{ color: area.color }" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <path v-if="area.id === 'amore'" d="M20.8 4.8a5.5 5.5 0 0 0-7.8 0L12 5.9l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 21l8.8-8.4a5.5 5.5 0 0 0 0-7.8Z" />
                <path v-else-if="area.id === 'capacita'" d="M12 2l1.8 6.2L20 10l-6.2 1.8L12 18l-1.8-6.2L4 10l6.2-1.8L12 2Zm6 12 .9 3.1L22 18l-3.1.9L18 22l-.9-3.1L14 18l3.1-.9L18 14Z" />
                <path v-else-if="area.id === 'mondo'" d="M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM3 12h18M12 3c2.4 2.5 3.5 5.5 3.5 9s-1.1 6.5-3.5 9c-2.4-2.5-3.5-5.5-3.5-9S9.6 5.5 12 3Z" />
                <path v-else d="M4 7h16a1 1 0 0 1 1 1v11H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h13M21 11h-5a2 2 0 0 0 0 4h5M16 13h.01" />
              </svg>
            </span><span class="area-label"><strong>{{ area.name }}</strong><small>{{ labels[values[area.id]] }}</small></span><span class="area-number">0{{ index + 1 }}</span>
          </button>
        </div>
        <div class="prism-reflection">
          <div><p class="prism-small">{{ current.name.toUpperCase() }}</p><p>{{ current.hint }}</p></div>
          <fieldset><legend>Come la senti oggi?</legend><div class="prism-choices"><button v-for="choice in (['non_definito', 'equilibrio', 'pressione'] as Feeling[])" :key="choice" type="button" :aria-pressed="values[selected] === choice" @click="values[selected] = choice">{{ labels[choice] }}</button></div></fieldset>
        </div>
        <p class="prism-note">La tua lettura resta in questa pagina e non viene salvata. Nessun valore è calcolato automaticamente.</p>
      </div>
    </div>
  </section>
</template>
