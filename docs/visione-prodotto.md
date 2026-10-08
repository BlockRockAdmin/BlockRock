# Visione di prodotto di BlockRock

## Scopo

BlockRock affianca una persona che attraversa una crisi prolungata e la aiuta a
ricostruire, passo dopo passo, una vita economica e personale che senta propria.
Non impone un percorso standard e non definisce la persona attraverso la crisi.

La metafora guida è quella di un pianeta che si è sganciato dalla propria orbita.
BlockRock offre un'orbita d'emergenza: un sostegno temporaneo che aiuta a
ritrovare stabilità e movimento, rispettando le caratteristiche individuali.
Il percorso dovrebbe lasciare spazio alla persona per ritrovare e scegliere la
propria direzione.

## Ripartenza attraverso le azioni

La persona non deve raccontare o spiegare verbalmente la propria situazione per
poter iniziare. Può scegliere piccoli gesti quotidiani che la aiutino a
riprendere contatto con le attività comuni, per esempio cucinare, uscire,
incontrare un amico o riprendere una routine di base.

Un gesto completato può essere registrato come un `+1`: indica un passo compiuto,
non un voto, una diagnosi o una misura del valore della persona. La persona
decide quali azioni hanno senso per lei; può ripeterle, cambiarle, saltarle o
fermarsi senza penalità. Il punteggio non deve diventare un obiettivo che
sostituisce il benessere della persona.

## Tre percorsi collegati, ma distinti

BlockRock considera tre dimensioni del recupero:

1. **Attività quotidiane:** piccoli passi scelti dalla persona per riaprire
   spazio alle abitudini e alle relazioni.
2. **Direzione personale:** riscoperta di interessi, capacità, valori e obiettivi
   propri. Studio, lavoro o altre roadmap entrano nel percorso quando la persona
   li sente pertinenti.
3. **Stabilità finanziaria:** ricostruzione graduale delle risorse e degli
   obiettivi economici, con eventuali strumenti finanziari presentati in modo
   comprensibile, facoltativo e sotto il controllo della persona.

I progressi in una dimensione non devono essere trattati automaticamente come
progressi nelle altre. In particolare, un `+1` relativo alle attività quotidiane
non deve modificare il capitale, aumentare il rischio o attivare operazioni di
Giano.

## Rapporto con Giano

BlockRock è concepito come copia shadow di Giano nell'ambito del percorso di
accumulo. La logica che collega lo stato del percorso alle operazioni deve essere
esplicita, verificabile e definita dalla scaletta approvata. Uno stato di crisi
non deve indurre il sistema ad aumentare il rischio nel tentativo di recuperare
più rapidamente.

Prima dell'implementazione vanno specificati i livelli della scaletta, le
transizioni consentite, i limiti operativi e il significato di ciascun livello.
La scala non va dedotta automaticamente da informazioni personali o da un
punteggio di attività.

## Confini del prodotto

BlockRock descrive difficoltà di vita dichiarate dalla persona, come problemi
finanziari, familiari o un lutto, e può adattare a esse il percorso di supporto.
Non assegna né presume diagnosi cliniche. Termini come depressione resistente,
depressione doppia e depressione bipolare appartengono all'ambito clinico e non
devono essere usati dall'app come stati dedotti o etichette per l'utente.

Le informazioni personali e finanziarie richiedono controllo e protezione
specifici. La blockchain non dovrebbe contenere dettagli personali della crisi;
un eventuale uso della catena va limitato a dati o attestazioni che la persona
abbia scelto consapevolmente di registrare.

## Primo perimetro funzionale

Un primo prototipo dovrebbe consentire alla persona di:

- scegliere un piccolo passo quotidiano e registrarlo volontariamente;
- definire interessi e obiettivi senza seguire un ordine prefissato;
- vedere un percorso economico spiegato e modificabile;
- controllare separatamente progressi personali e andamento finanziario;
- mettere in pausa o cambiare il percorso senza perdere i propri dati.

La scaletta operativa di Giano e le regole di accumulo restano da definire prima
di implementare qualsiasi automazione finanziaria.
