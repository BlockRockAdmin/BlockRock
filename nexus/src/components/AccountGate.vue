<script setup lang="ts">
import { ref } from "vue";
const props = defineProps<{
  busy: boolean;
  registrationEnabled: boolean;
  renewUsername?: string;
}>();
const emit = defineEmits<{
  authenticate: [mode: "login" | "register" | "recover", form: Record<string, string>];
}>();
const mode = ref<"login" | "register" | "recover">("login");
const username = ref(props.renewUsername ?? "");
const displayName = ref("");
const password = ref("");
const invitation = ref("");
const recoveryCode = ref("");
function submit() {
  const form: Record<string, string> = {
    username: username.value.trim(),
    password: password.value,
  };
  if (mode.value === "register")
    Object.assign(form, { displayName: displayName.value.trim(), invitation: invitation.value });
  if (mode.value === "recover") form.recoveryCode = recoveryCode.value.trim();
  emit("authenticate", mode.value, form);
  password.value = "";
  invitation.value = "";
  recoveryCode.value = "";
}
</script>

<template>
  <div class="access-layout">
    <section class="access-intro">
      <p class="eyebrow">BlockRock / Nexus</p>
      <h1>Riparti da<br /><span>una cosa tua.</span></h1>
      <p class="lead">
        Un piccolo gesto, un interesse da ritrovare, una direzione da scegliere. Il percorso resta
        tuo, anche quando cambi dispositivo.
      </p>
      <div class="orbit" aria-hidden="true"><span class="orbit-core">+1</span><i></i><b></b></div>
      <div class="principles">
        <span>01 · Il tuo ritmo</span><span>02 · Le tue scelte</span><span>03 · I tuoi dati</span>
      </div>
    </section>
    <section class="card access-card" aria-labelledby="access-title">
      <p class="eyebrow">Il tuo spazio personale</p>
      <h2 id="access-title">
        {{
          renewUsername
            ? "Rinnova l’accesso"
            : mode === "register"
              ? "Crea il tuo account"
              : mode === "recover"
                ? "Ritrova il tuo spazio"
                : "Bentornato in Nexus"
        }}
      </h2>
      <p class="muted">
        {{
          mode === "recover"
            ? "Usa il codice personale salvato alla creazione dell’account. Dopo il recupero ne riceverai uno nuovo."
            : "Accedi al servizio privato per ritrovare passi e obiettivi sui tuoi dispositivi."
        }}
      </p>
      <div v-if="!renewUsername" class="segmented" aria-label="Accesso account">
        <button :aria-pressed="mode === 'login'" :disabled="busy" @click="mode = 'login'">
          Accedi
        </button>
        <button
          :aria-pressed="mode === 'register'"
          :disabled="busy || !registrationEnabled"
          @click="mode = 'register'"
        >
          Crea account
        </button>
      </div>
      <p v-if="!registrationEnabled && !renewUsername" class="muted small">
        Le registrazioni sono chiuse. Il proprietario del server può abilitare gli inviti.
      </p>
      <form class="form-stack" @submit.prevent="submit">
        <label v-if="mode === 'register'"
          >Come vuoi essere chiamato<input
            v-model="displayName"
            required
            maxlength="60"
            autocomplete="nickname"
        /></label>
        <label
          >Nome utente<input
            v-model="username"
            required
            minlength="3"
            maxlength="40"
            pattern="[a-zA-Z0-9._\-]+"
            autocomplete="username"
            autocapitalize="none"
            :readonly="!!renewUsername"
            spellcheck="false"
          /><span class="hint"
            >Lettere, numeri, punti, trattini. Nessuna email richiesta.</span
          ></label
        >
        <label v-if="mode === 'recover'"
          >Codice di recupero<input
            v-model="recoveryCode"
            required
            maxlength="200"
            autocomplete="off"
            spellcheck="false"
        /></label>
        <label
          >{{ mode === "recover" ? "Nuova password" : "Password"
          }}<input
            v-model="password"
            required
            type="password"
            minlength="12"
            maxlength="128"
            :autocomplete="mode === 'login' ? 'current-password' : 'new-password'"
          /><span class="hint">Almeno 12 caratteri.</span></label
        >
        <label v-if="mode === 'register'"
          >Codice di invito<input
            v-model="invitation"
            required
            maxlength="200"
            type="password"
            autocomplete="off"
        /></label>
        <button type="submit" class="primary" :disabled="busy">
          {{
            busy
              ? "Un momento…"
              : mode === "register"
                ? "Crea il mio spazio"
                : mode === "recover"
                  ? "Recupera account"
                  : "Entra nel tuo spazio"
          }}
          <span aria-hidden="true">↗</span>
        </button>
      </form>
      <button
        v-if="!renewUsername"
        class="text-button"
        :disabled="busy"
        @click="mode = mode === 'recover' ? 'login' : 'recover'"
      >
        {{ mode === "recover" ? "Torna all’accesso" : "Hai dimenticato la password?" }}
      </button>
      <p class="privacy-note">
        I dati sono conservati nel tuo servizio Nexus e sincronizzati solo con il tuo account. Puoi
        esportarli o eliminarli quando vuoi.
      </p>
    </section>
  </div>
</template>
