import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

const password = "nexus-test-password-123";
let accountNumber = 0;
async function register(page: Page) {
  const username = `test-${Date.now()}-${accountNumber++}`;
  await page.goto("/");
  await page.getByRole("button", { name: "Crea account", exact: true }).click();
  await page.getByLabel("Come vuoi essere chiamato").fill("Andrea");
  await page.getByLabel("Nome utente").fill(username);
  await page.getByLabel("Password", { exact: false }).fill(password);
  await page.getByLabel("Codice di invito").fill("nexus-e2e-invitation-not-a-real-secret");
  await page.getByRole("button", { name: "Crea il mio spazio" }).click();
  await expect(
    page.getByRole("heading", { name: "Conserva la chiave del tuo spazio." }),
  ).toBeVisible();
  const recovery = (await page.locator(".recovery-card code").textContent())!;
  await page.getByRole("button", { name: "L’ho conservato, continua" }).click();
  await expect(page.getByRole("button", { name: "Ho cucinato qualcosa" })).toBeEnabled();
  return { username, recovery };
}
async function login(page: Page, username: string, nextPassword = password) {
  await page.goto("/");
  await page.getByLabel("Nome utente").fill(username);
  await page.getByLabel("Password", { exact: false }).fill(nextPassword);
  await page.getByRole("button", { name: "Entra nel tuo spazio" }).click();
  await expect(page.getByRole("button", { name: "Ho cucinato qualcosa" })).toBeEnabled();
}
async function record(page: Page, action: string) {
  await page.getByLabel("Oppure un gesto tuo").fill(action);
  await page.getByRole("button", { name: "Segna +1", exact: true }).click();
  await expect(page.locator(".entries strong", { hasText: action })).toBeVisible();
  await expect(page.getByRole("button", { name: "Sincronizza ora" })).toBeEnabled();
}
async function dataPage(page: Page) {
  await page.getByRole("button", { name: "Dati e account", exact: true }).click();
}
async function exportData(page: Page) {
  const downloading = page.waitForEvent("download");
  await page.getByRole("button", { name: "Scarica il mio percorso" }).click();
  const path = await (await downloading).path();
  return JSON.parse(await readFile(path!, "utf8"));
}

test("passi, modifica, rimozione annullabile, obiettivi separati e pausa", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await register(page);
  await record(page, "Una passeggiata");
  await page.getByRole("button", { name: "Modifica: Una passeggiata", exact: true }).click();
  await page.getByLabel("Modifica gesto").fill("Una passeggiata al parco");
  await page.getByRole("button", { name: "Salva gesto", exact: true }).click();
  await expect(page.locator(".entries strong")).toHaveText("Una passeggiata al parco");
  await page
    .getByRole("button", { name: "Rimuovi: Una passeggiata al parco", exact: true })
    .click();
  await expect(page.locator(".entries strong")).toHaveCount(0);
  await page.getByRole("button", { name: "Annulla ultima modifica" }).click();
  await expect(page.locator(".entries strong")).toHaveText("Una passeggiata al parco");
  await page.getByRole("button", { name: "La mia direzione", exact: true }).click();
  await page.getByLabel("Un tuo interesse").fill("Musica");
  await page.getByRole("button", { name: "Aggiungi interesse", exact: true }).click();
  await expect(page.locator(".chips")).toContainText("Musica");
  await page.getByLabel("Cosa vuoi provare a fare?").fill("Riprendere la chitarra");
  await page.getByLabel("Qual è il prossimo piccolo passo?").fill("Dieci minuti, quando mi va");
  await page.getByRole("button", { name: "Aggiungi obiettivo", exact: true }).click();
  await page.getByRole("button", { name: "Segna come concluso" }).click();
  await expect(page.locator(".goal-card")).toContainText("Concluso, a modo tuo");
  await page.getByRole("button", { name: "Riapri", exact: true }).click();
  await expect(page.locator(".goal-card")).toContainText("Una direzione aperta");
  await page.getByRole("button", { name: "Stabilità", exact: true }).click();
  await expect(page.locator(".goal-card")).toHaveCount(0);
  await page.getByLabel("Cosa vuoi provare a fare?").fill("Capire le mie spese");
  await page.getByRole("button", { name: "Aggiungi obiettivo", exact: true }).click();
  await dataPage(page);
  await page.getByRole("button", { name: "Metti in pausa", exact: true }).click();
  await page.getByRole("button", { name: "Oggi", exact: true }).click();
  await expect(page.getByRole("button", { name: "Ho cucinato qualcosa" })).toBeDisabled();
  await page.getByRole("button", { name: "Riprendi il percorso", exact: true }).click();
  await expect(page.getByRole("button", { name: "Ho cucinato qualcosa" })).toBeEnabled();
  await page.screenshot({ path: "test-results/nexus-desktop.png", fullPage: true });
  expect(errors).toEqual([]);
});

test("due dispositivi: sincronizzazione, bozza offline, conflitto e scelta della versione", async ({
  browser,
  page,
}) => {
  const account = await register(page);
  await record(page, "Primo dispositivo");
  const second = await browser.newContext();
  const other = await second.newPage();
  try {
    await login(other, account.username);
    await expect(other.locator(".entries strong")).toHaveText("Primo dispositivo");
    await record(other, "Secondo dispositivo");
    await page.getByRole("button", { name: "Sincronizza ora" }).click();
    await expect(page.locator(".entries strong")).toHaveCount(2);
    await page.context().setOffline(true);
    await record(page, "Bozza da conservare");
    await expect(page.getByRole("alert")).toContainText("Servizio non raggiungibile");
    await record(other, "Una modifica dal server");
    await page.context().setOffline(false);
    await page.getByRole("button", { name: "Sincronizza ora" }).click();
    await expect(
      page.getByRole("heading", { name: "Conserviamo la versione giusta." }),
    ).toBeVisible();
    await page.reload();
    await expect(page.locator(".entries")).toContainText("Bozza da conservare");
    await expect(
      page.getByRole("heading", { name: "Conserviamo la versione giusta." }),
    ).toBeVisible();
    await page.getByLabel("Voglio sostituire il percorso sul server").check();
    await page
      .getByRole("button", { name: "Sostituisci la versione sul server", exact: true })
      .click();
    await expect(page.locator(".conflict-card")).toHaveCount(0);
    await other.getByRole("button", { name: "Sincronizza ora" }).click();
    await expect(other.locator(".entries")).toContainText("Bozza da conservare");
    await expect(other.locator(".entries")).not.toContainText("Una modifica dal server");
  } finally {
    await second.close();
  }
});

test("esporta, anteprima, ripristina, migra i vecchi passi e rifiuta file corrotti", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      "blockrock.recovery-steps.v1",
      JSON.stringify([
        { id: "legacy-1", action: "Vecchio gesto", createdAt: "2026-11-01T08:00:00Z" },
      ]),
    ),
  );
  await register(page);
  await record(page, "Gesto da esportare");
  await dataPage(page);
  const backup = await exportData(page);
  expect(backup.journey.steps).toHaveLength(1);
  expect(JSON.stringify(backup)).not.toContain(password);
  await page.getByLabel("Scegli un backup Nexus").setInputFiles({
    name: "backup.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(backup)),
  });
  await expect(page.locator(".import-preview")).toContainText("1 passi");
  await page.getByRole("button", { name: "Conferma ripristino" }).click();
  expect((await exportData(page)).journey.steps).toHaveLength(1);
  await page.getByLabel("Scegli un backup Nexus").setInputFiles({
    name: "rotto.json",
    mimeType: "application/json",
    buffer: Buffer.from("{rotto"),
  });
  await expect(page.locator(".import-card [role=alert]")).toBeVisible();
  expect((await exportData(page)).journey.steps).toHaveLength(1);
  await page.getByRole("button", { name: "Esamina i vecchi passi" }).click();
  await page.getByRole("button", { name: "Conferma ripristino" }).click();
  expect((await exportData(page)).journey.steps).toHaveLength(2);
  expect(
    await page.evaluate(() => localStorage.getItem("blockrock.recovery-steps.v1")),
  ).not.toBeNull();
  const replacement = { ...backup, journey: { ...backup.journey, paused: true, steps: [] } };
  await page.getByLabel("Scegli un backup Nexus").setInputFiles({
    name: "sostituzione.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(replacement)),
  });
  await page.getByLabel("Sostituisci tutto il percorso con questo file").check();
  await page.getByRole("button", { name: "Conferma ripristino" }).click();
  const restored = await exportData(page);
  expect(restored.journey.steps).toHaveLength(0);
  expect(restored.journey.paused).toBe(true);
});

test("recupero account, cambio password, logout e cancellazione", async ({ page }) => {
  const account = await register(page);
  await record(page, "Un passo da ritrovare");
  await page.getByRole("button", { name: "Esci", exact: true }).click();
  await page.getByRole("button", { name: "Hai dimenticato la password?" }).click();
  await page.getByLabel("Nome utente").fill(account.username);
  await page.getByLabel("Codice di recupero", { exact: true }).fill(account.recovery);
  await page.getByLabel("Nuova password", { exact: false }).fill("recovered-password-123");
  await page.getByRole("button", { name: "Recupera account", exact: true }).click();
  await expect(page.locator(".recovery-card code")).not.toHaveText(account.recovery);
  await page.getByRole("button", { name: "L’ho conservato, continua" }).click();
  await expect(page.locator(".entries")).toContainText("Un passo da ritrovare");
  await dataPage(page);
  await page.getByLabel("Password attuale").fill("recovered-password-123");
  await page.getByLabel("Nuova password", { exact: true }).fill("changed-password-1234");
  await page.getByRole("button", { name: "Aggiorna password", exact: true }).click();
  await expect(page.locator(".recovery-card")).toBeVisible();
  await page.getByRole("button", { name: "L’ho conservato, continua" }).click();
  await page.getByRole("button", { name: "Esci", exact: true }).click();
  await login(page, account.username, "changed-password-1234");
  await dataPage(page);
  await page.getByRole("button", { name: "Voglio eliminare l’account" }).click();
  await page.getByLabel("Password per confermare").fill("changed-password-1234");
  await page.getByLabel("Scrivi ELIMINA").fill("ELIMINA");
  await page.getByRole("button", { name: "Elimina definitivamente", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Bentornato in Nexus" })).toBeVisible();
  await expect(page.getByRole("status")).toContainText("Account e percorso eliminati");
});

test("interfaccia mobile: nessun overflow, accesso e navigazione", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.screenshot({ path: "test-results/nexus-mobile-accesso.png", fullPage: true });
  await register(page);
  await record(page, "Un gesto dal telefono");
  for (const name of ["La mia direzione", "Stabilità", "Dati e account", "Oggi"]) {
    await page.getByRole("button", { name, exact: true }).click();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  }
  await page.screenshot({ path: "test-results/nexus-mobile.png", fullPage: true });
});

test("storage pieno: segnala la bozza in memoria e ritrova i passi dopo il rinnovo della sessione", async ({
  browser,
  page,
}) => {
  const account = await register(page);
  await page.evaluate(() => {
    Storage.prototype.setItem = () => {
      throw new DOMException("Quota piena", "QuotaExceededError");
    };
  });
  await page.context().setOffline(true);
  await record(page, "Bozza senza spazio locale");
  await expect(page.getByRole("status").filter({ hasText: "Il browser non riesce" })).toBeVisible();
  const otherContext = await browser.newContext();
  const other = await otherContext.newPage();
  try {
    await login(other, account.username);
    await dataPage(other);
    await other.getByLabel("Password attuale").fill(password);
    await other.getByLabel("Nuova password", { exact: true }).fill("quota-renew-password-123");
    await other.getByRole("button", { name: "Aggiorna password", exact: true }).click();
    await expect(other.locator(".recovery-card")).toBeVisible();
    await page.context().setOffline(false);
    await page.getByRole("button", { name: "Sincronizza ora" }).click();
    await expect(page.getByRole("heading", { name: "Rinnova l’accesso" })).toBeVisible();
    await page.getByLabel("Password", { exact: false }).fill("quota-renew-password-123");
    await page.getByRole("button", { name: "Entra nel tuo spazio" }).click();
    await expect(page.locator(".entries")).toContainText("Bozza senza spazio locale");
    await page.getByRole("button", { name: "Sincronizza ora" }).click();
    await expect(page.locator(".sync-bar")).toContainText("Sincronizzato");
  } finally {
    await otherContext.close();
  }
});

test("due schede: evita la sovrascrittura di una bozza locale altrui", async ({
  context,
  page,
}) => {
  await register(page);
  const other = await context.newPage();
  await other.goto("/");
  await expect(other.getByRole("button", { name: "Ho cucinato qualcosa" })).toBeEnabled();
  await context.setOffline(true);
  await record(page, "Bozza prima scheda");
  await other.getByLabel("Oppure un gesto tuo").fill("Bozza seconda scheda");
  await other.getByRole("button", { name: "Segna +1", exact: true }).click();
  await expect(
    other.getByRole("heading", { name: "Conserviamo la versione giusta." }),
  ).toBeVisible();
  const stored = await other.evaluate(() =>
    Object.entries(localStorage)
      .filter(([key]) => key.startsWith("blockrock.nexus.draft.v2."))
      .map(([, value]) => value),
  );
  expect(stored.join()).toContain("Bozza prima scheda");
  expect(stored.join()).not.toContain("Bozza seconda scheda");
  // Entrambe le versioni restano esportabili, nessuna scrittura sul server è partita.
  await context.setOffline(false);
  await page.getByRole("button", { name: "Sincronizza ora" }).click();
  await expect(page.locator(".sync-bar")).toContainText("Sincronizzato");
  await other.getByRole("button", { name: "Carica server e scarta le modifiche locali" }).click();
  await expect(other.locator(".entries")).toContainText("Bozza prima scheda");
  await expect(other.locator(".entries")).not.toContainText("Bozza seconda scheda");
});

test("storage non disponibile: account e salvataggio sul server funzionano comunque", async ({
  page,
}) => {
  await page.addInitScript(() => {
    for (const method of ["getItem", "setItem", "removeItem"] as const) {
      Storage.prototype[method] = () => {
        throw new DOMException("Storage disabilitato", "SecurityError");
      };
    }
  });
  const account = await register(page);
  await expect(
    page.getByRole("status").filter({ hasText: "Il browser non permette" }),
  ).toBeVisible();
  await record(page, "Passo senza storage del browser");
  await expect(page.locator(".sync-bar")).toContainText("Sincronizzato");
  await page.getByRole("button", { name: "Esci", exact: true }).click();
  await login(page, account.username);
  await expect(page.locator(".entries")).toContainText("Passo senza storage del browser");
});
