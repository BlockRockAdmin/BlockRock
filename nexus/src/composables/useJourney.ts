import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  clone,
  download,
  emptyJourney,
  exportJourney,
  parseJourney,
  parseSnapshot,
} from "../domain/journey";
import type { Journey, Snapshot } from "../domain/journey";

type User = { id: string; username: string; displayName: string };
type Session = { user: User; csrf: string; recoveryCode?: string };
class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}
export function useJourney() {
  const user = ref<User | null>(null);
  const journey = ref<Journey>(emptyJourney());
  const loading = ref(true);
  const busy = ref(false);
  const pending = ref(false);
  const conflict = ref(false);
  const error = ref("");
  const notice = ref("");
  const recoveryCode = ref("");
  const registrationEnabled = ref(false);
  const sessionExpired = ref(false);
  const loaded = ref(false);
  const lastSynced = ref("");
  const undoState = ref<Journey | null>(null);
  let csrf = "";
  let revision = 0;
  let draftRaw: string | null = null;
  let timer: ReturnType<typeof setInterval> | undefined;
  const draftKey = () => `blockrock.nexus.draft.v2.${user.value?.id}`;
  const canEdit = computed(
    () =>
      !!user.value &&
      loaded.value &&
      !loading.value &&
      !busy.value &&
      !conflict.value &&
      !sessionExpired.value,
  );
  const syncLabel = computed(() =>
    conflict.value
      ? "Versioni da confrontare"
      : busy.value
        ? "Sincronizzazione…"
        : pending.value
          ? "Modifiche da inviare"
          : "Sincronizzato",
  );

  async function api<T>(path: string, method = "GET", body?: unknown): Promise<T> {
    const response = await fetch(`/api/${path}`, {
      method,
      credentials: "same-origin",
      cache: "no-store",
      signal: AbortSignal.timeout(12000),
      headers: {
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
        ...(csrf ? { "X-Nexus-CSRF": csrf } : {}),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const result = await response.json();
    if (!response.ok)
      throw new ApiError(
        response.status,
        typeof result.detail === "string" ? result.detail : "Controlla i dati inseriti e riprova.",
      );
    return result as T;
  }
  function message(reason: unknown) {
    return reason instanceof ApiError
      ? reason.message
      : "Servizio non raggiungibile. Le modifiche restano su questo dispositivo; riprova la sincronizzazione.";
  }
  function applySnapshot(snapshot: Snapshot) {
    journey.value = clone(snapshot.journey);
    revision = snapshot.revision;
    lastSynced.value = snapshot.updatedAt;
  }
  function saveDraft() {
    if (!user.value) return;
    try {
      const previous = localStorage.getItem(draftKey());
      if (previous !== draftRaw) {
        conflict.value = true;
        error.value =
          "Un’altra scheda ha modifiche locali. Esporta questa versione e ricarica per leggere quella scheda.";
        return;
      }
      const raw = JSON.stringify({
        journey: journey.value,
        revision,
        updatedAt: new Date().toISOString(),
      });
      localStorage.setItem(draftKey(), raw);
      draftRaw = raw;
    } catch {
      notice.value =
        "Il browser non riesce a salvare una copia locale. Esporta le modifiche prima di chiudere.";
    }
  }
  function clearDraft() {
    try {
      if (localStorage.getItem(draftKey()) === draftRaw) localStorage.removeItem(draftKey());
      else if (localStorage.getItem(draftKey()) !== null) {
        conflict.value = true;
        error.value = "Un’altra scheda conserva modifiche locali. Ricarica per leggerle.";
      }
    } catch {
      /* La copia sul server rimane disponibile. */
    }
    draftRaw = null;
  }
  async function load() {
    const snapshot = parseSnapshot(await api("journey"));
    applySnapshot(snapshot);
    loaded.value = true;
    try {
      draftRaw = localStorage.getItem(draftKey());
    } catch {
      draftRaw = null;
      notice.value =
        "Il browser non permette copie locali. Durante un’interruzione di rete esporta le modifiche prima di chiudere.";
      return;
    }
    try {
      if (draftRaw) {
        const draft = parseSnapshot(JSON.parse(draftRaw));
        if (JSON.stringify(draft.journey) !== JSON.stringify(snapshot.journey)) {
          journey.value = draft.journey;
          pending.value = true;
          conflict.value = draft.revision !== snapshot.revision;
          revision = draft.revision;
          notice.value = "Ho recuperato modifiche non ancora inviate da questo browser.";
        } else clearDraft();
      }
    } catch {
      conflict.value = true;
      error.value =
        "La copia locale non è leggibile. Puoi scaricarla prima di usare i dati del server.";
    }
  }
  async function boot() {
    loading.value = true;
    error.value = "";
    try {
      registrationEnabled.value = (
        await api<{ registrationEnabled: boolean }>("config")
      ).registrationEnabled;
      const session = await api<Session>("session");
      user.value = session.user;
      csrf = session.csrf;
      await load();
    } catch (reason) {
      if (!(reason instanceof ApiError && reason.status === 401)) error.value = message(reason);
    } finally {
      loading.value = false;
    }
  }
  async function authenticate(
    mode: "login" | "register" | "recover",
    form: Record<string, string>,
  ) {
    busy.value = true;
    error.value = "";
    notice.value = "";
    try {
      // Anche quando lo storage è pieno, il rinnovo della sessione conserva la bozza in memoria.
      const previous =
        user.value && pending.value
          ? { userId: user.value.id, journey: clone(journey.value), revision }
          : null;
      const session = await api<Session>(mode, "POST", form);
      user.value = session.user;
      csrf = session.csrf;
      recoveryCode.value = session.recoveryCode ?? "";
      sessionExpired.value = false;
      pending.value = false;
      conflict.value = false;
      undoState.value = null;
      loaded.value = false;
      await load();
      if (
        previous &&
        previous.userId === session.user.id &&
        JSON.stringify(previous.journey) !== JSON.stringify(journey.value)
      ) {
        const serverRevision = revision;
        journey.value = previous.journey;
        revision = previous.revision;
        pending.value = true;
        conflict.value = conflict.value || revision !== serverRevision;
        saveDraft();
      }
    } catch (reason) {
      error.value = message(reason);
    } finally {
      busy.value = false;
    }
  }
  async function sync() {
    if (!user.value || busy.value || loading.value || conflict.value) return;
    busy.value = true;
    try {
      if (pending.value) {
        const result = await api<{ revision: number; updatedAt: string }>("journey", "PUT", {
          revision,
          journey: journey.value,
        });
        revision = result.revision;
        lastSynced.value = result.updatedAt;
        pending.value = false;
        clearDraft();
      } else {
        const result = parseSnapshot(await api("journey"));
        if (!loaded.value) await load();
        else if (result.revision !== revision) {
          applySnapshot(result);
          undoState.value = null;
        }
      }
      if (!conflict.value) error.value = "";
    } catch (reason) {
      if (reason instanceof ApiError && reason.status === 409) conflict.value = true;
      if (reason instanceof ApiError && reason.status === 401) sessionExpired.value = true;
      error.value = message(reason);
    } finally {
      busy.value = false;
    }
  }
  async function change(update: (value: Journey) => void, announcement: string) {
    if (!canEdit.value) return;
    try {
      const next = clone(journey.value);
      update(next);
      const validated = parseJourney(next);
      undoState.value = clone(journey.value);
      journey.value = validated;
      pending.value = true;
      notice.value = announcement;
      error.value = "";
      saveDraft();
      await sync();
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : "Modifica non valida.";
    }
  }
  async function undo() {
    if (!undoState.value) return;
    const previous = clone(undoState.value);
    await change((value) => Object.assign(value, previous), "Ultima modifica annullata.");
    undoState.value = null;
  }
  async function resolveConflict(useLocal: boolean) {
    busy.value = true;
    try {
      const server = parseSnapshot(await api("journey"));
      if (useLocal) {
        const result = await api<{ revision: number; updatedAt: string }>("journey", "PUT", {
          revision: server.revision,
          journey: journey.value,
        });
        revision = result.revision;
        lastSynced.value = result.updatedAt;
      } else applySnapshot(server);
      // Scelta esplicita: elimina anche la bozza di un'altra scheda solo qui.
      try {
        localStorage.removeItem(draftKey());
      } catch {
        /* La versione scelta è già sul server. */
      }
      draftRaw = null;
      conflict.value = false;
      pending.value = false;
      error.value = "";
      undoState.value = null;
      notice.value = useLocal
        ? "Versione di questo dispositivo inviata."
        : "Versione del server caricata.";
    } catch (reason) {
      error.value = message(reason);
    } finally {
      busy.value = false;
    }
  }
  function exportData() {
    download(`nexus-${new Date().toISOString().slice(0, 10)}.json`, exportJourney(journey.value));
  }
  function exportDraft() {
    if (draftRaw) download("nexus-copia-locale-da-verificare.json", draftRaw);
    else exportData();
  }
  async function logout() {
    if (busy.value || pending.value || conflict.value) {
      error.value =
        "Prima di uscire, sincronizza o risolvi le modifiche in sospeso. Puoi anche esportarle.";
      return;
    }
    busy.value = true;
    try {
      await api("logout", "POST", {});
      clearDraft();
      user.value = null;
      csrf = "";
      loaded.value = false;
      journey.value = emptyJourney();
      undoState.value = null;
      recoveryCode.value = "";
      notice.value = "";
      error.value = "";
    } catch (reason) {
      error.value = message(reason);
    } finally {
      busy.value = false;
    }
  }
  async function changePassword(currentPassword: string, password: string) {
    busy.value = true;
    error.value = "";
    try {
      const result = await api<{ csrf: string; recoveryCode: string }>("password", "POST", {
        currentPassword,
        password,
      });
      csrf = result.csrf;
      recoveryCode.value = result.recoveryCode;
      notice.value = "Password aggiornata. Gli altri dispositivi devono accedere di nuovo.";
    } catch (reason) {
      error.value = message(reason);
    } finally {
      busy.value = false;
    }
  }
  async function deleteAccount(password: string) {
    busy.value = true;
    error.value = "";
    try {
      await api("account/delete", "POST", { password });
      try {
        localStorage.removeItem(draftKey());
      } catch {
        /* L'account sul server è già stato eliminato. */
      }
      draftRaw = null;
      user.value = null;
      csrf = "";
      journey.value = emptyJourney();
      pending.value = false;
      conflict.value = false;
      undoState.value = null;
      recoveryCode.value = "";
      notice.value =
        "Account e percorso eliminati dal servizio. Le copie esportate e i backup del server vanno eliminati separatamente.";
    } catch (reason) {
      error.value = message(reason);
    } finally {
      busy.value = false;
    }
  }
  function beforeUnload(event: BeforeUnloadEvent) {
    if (pending.value || conflict.value) {
      event.preventDefault();
      event.returnValue = "";
    }
  }
  function storageChanged(event: StorageEvent) {
    if (user.value && event.key === draftKey() && event.newValue !== draftRaw) {
      if (pending.value) {
        conflict.value = true;
        error.value =
          "Un’altra scheda sta modificando il percorso. Esporta le modifiche e scegli la versione da conservare.";
      } else void sync();
    }
  }
  onMounted(() => {
    void boot();
    timer = setInterval(() => {
      if (!document.hidden) void sync();
    }, 15000);
    window.addEventListener("beforeunload", beforeUnload);
    window.addEventListener("storage", storageChanged);
  });
  onUnmounted(() => {
    clearInterval(timer);
    window.removeEventListener("beforeunload", beforeUnload);
    window.removeEventListener("storage", storageChanged);
  });
  return {
    user,
    journey,
    loading,
    busy,
    pending,
    conflict,
    error,
    notice,
    recoveryCode,
    registrationEnabled,
    sessionExpired,
    lastSynced,
    canEdit,
    syncLabel,
    undoState,
    boot,
    authenticate,
    sync,
    change,
    undo,
    resolveConflict,
    exportData,
    exportDraft,
    logout,
    changePassword,
    deleteAccount,
  };
}
