# Pravidla

Jak se Dostihy a sázky hrají v této hře: pravidla deskové hry tak, jak je
hrajeme doma, domácí pravidla a jak hraje počítač. Klidně je upravujte – kód
se řídí tímto souborem a tento soubor kódem (viz `AGENTS.md`). Čísla
v tabulkách a texty karet hlídá `tests/rules_doc.rs` proti `src/data.rs`.

## Čísla

| Co | Konstanta | Částka |
|---|---|--:|
| Peníze na začátku hry | `START_MONEY` | 30 000 |
| Průchod startem (i zastavení na něm) | `START_BONUS` | 4 000 |
| Navíc za zastavení přímo na startu hodem kostkou | `START_LANDING_BONUS` | 1 000 |
| Cena trenéra | `TRAINER_PRICE` | 4 000 |
| Cena Přepravy a Stájí | `SERVICE_PRICE` | 3 000 |
| Poplatek za trenéra, za každého trenéra majitele | `TRAINER_FEE` | 1 000 |
| Poplatek za Přepravu či Stáje, krát hod kostkou | `SERVICE_FEE` | 80 |
| Totéž, když má majitel obojí | `BOTH_SERVICES_FEE` | 200 |
| Pokusy za tah hodit šestku na distancu | `DISTANC_TRIES` | 3 |
| Nabídky obchodu, které může mít hráč zároveň | `MAX_TRADE_OFFERS` | 3 |

## Hra

Hrají 2 až 6 hráčů, každý člověk nebo počítač, v pořadí figurek (Červený,
Žlutý, Zelený, Modrý, Fialový, Bílý – kdo hraje). Všichni začínají na startu
s penězi na začátek hry. Oba balíčky karet se zamíchají.

Hra končí, když zbude jediný hráč – ostatní zbankrotovali. Kdo nemá čas, může
hru kdykoli ukončit: vyhrává nejbohatší, počítají se peníze, ceny jeho polí
a dostihů zaplacených na jeho koně (karta *Zrušen distanc* se nepočítá).

## Tah

1. **Před hodem** smí hráč, který čeká na distancu, použít kartu *Zrušen
   distanc* a podle domácího pravidla koupit volné pole, na kterém stojí.
2. **Hod.** Hází se jednou kostkou; po šestce hází znovu a hody se sčítají.
   Tři šestky za sebou pošlou figurku rovnou na distanc (bez pohybu a bez
   peněz za start).
   - Kdo je na distancu, má tři pokusy hodit šestku. Šestka ho pustí a hází
     znovu normálně (o tu šestku nejde). Bez šestky zůstává a tah pokračuje
     bez pohybu.
3. **Pohyb** po směru hodinových ručiček o součet hodů. Průchod startem nebo
   zastavení na něm dává 4 000, zastavení přímo na startu hodem kostkou ještě
   1 000 navíc.
4. **Pole**, na kterém figurka zastaví, platí (viz dál).
5. **Po pohybu** smí hráč v libovolném pořadí koupit volné pole, na kterém
   zastavil, koupit dostihy koni, na kterém zastavil, nabízet obchody
   a prodávat bance pole a dostihy. Tah končí, když to hráč řekne – ale ne,
   dokud dluží.

Kdo stojí (karta Náhoda nebo doping), nehází a netáhne: jeho tah začíná
rovnou bodem 5.

## Pole

- **Nikoho kůň, trenér, Přeprava či Stáje** – hráč je smí koupit za jejich
  cenu, dokud neskončí jeho tah. Nekoupit nemusí nijak potvrzovat.
- **Vlastní pole** – nic se neplatí. Na vlastním koni z celé stáje smí hráč
  koupit dostihy (viz Dostihy).
- **Cizí kůň** – majiteli se platí poplatek podle dostihů koně (tabulka
  hracího plánu). Když majitel čeká na distancu nebo je podezřelý z dopingu,
  jeho koně vydělávají jen prohlídku stáje.
- **Cizí trenér** – 1 000 za každého trenéra, kterého má majitel.
- **Cizí Přeprava či Stáje** – 80× poslední hod kostkou, 200×, když má
  majitel obojí. Kdo se tam dostal kartou, platí podle hodu z toho tahu.
- **Finance, Náhoda** – hráč táhne kartu z balíčku a řídí se jí.
- **Veterinární vyšetření** – hráč zaplatí poplatek bance.
- **Distanc** – figurka tam čeká (viz Hod).
- **Podezření z dopingu** – hráč jeden tah stojí a jeho koně mezitím
  vydělávají jen prohlídku stáje.
- **Parkoviště** – nic (ale viz domácí pravidla).
- **Start** – jen peníze za start.

## Dostihy

Kdo má všechny koně jedné stáje, může své koně posílat na dostihy. Dostihy
koni kupuje, když na něm zastaví, jednou za návštěvu: jeden až čtyři dostihy
– nejvýš čtyři na koni – nebo, když už má čtyři, hlavní dostih sezóny.
Hlavní dostih tedy chce aspoň druhou návštěvu. Každý dostih, i hlavní, stojí,
kolik stojí dostih té stáje. Víc dostihů znamená vyšší poplatky.

Dostihy se vracejí bance po jednom za polovinu ceny dostihu (hlavní dostih
se vrací na čtyři dostihy). Stáj s dostihy se nedělí: bance se její pole
prodávají, až když dostihy zmizí, a v obchodě mění majitele jen všechna
najednou. Kůň, který se vrátí bance, přichází o dostihy.

## Karty

Tažená karta jde pod svůj balíček – kromě *Zrušen distanc*, kterou si hráč
nechá.

- *Zrušen distanc* se hraje před hodem při čekání na distancu: figurka je
  volná a hází normálně. Dá se vyměnit v obchodě jako pole. Po použití, nebo
  když její držitel zbankrotuje, jde pod balíček Náhoda.
- Karta, která pohne figurkou, nechá platit pole, kde figurka skončí. Průchod
  startem cestou (i pozpátku) dává peníze za start, pokud karta neříká jinak.
  1 000 za zastavení přímo na startu je jen za hod kostkou.
- „Mimořádný zisk z dostihů“ platí jen hráči, který má koně s dostihy.
- „Renovuješ všechny stáje…“ počítá každý dostih na koních hráče; kůň
  v hlavním dostihu se počítá jednou, za cenu hlavního dostihu.
- „…od každého hráče…“ – platí všichni ostatní hráči.

Karty Finance:

- „Zaplať pojistku 1000,-“
- „Pokuta za nedodržení předpisů. Zaplať 400,-“
- „Renovuješ všechny stáje. Za každý svůj obsazený dostih zaplať 500,-“
- „Mimořádný zisk z dostihů. Obdržíš 2000,-“
- „Jako dárek k narozeninám obdržíš od každého hráče 200,-“
- „Mimořádná prémie 500,-“
- „Obdržíš dotaci 4000,-“
- „Zaplať dluh 3000,-“
- „Renovuješ všechny stáje. Za každý svůj dostih zaplať 800,-, za každý hlavní dostih 2300,-“
- „Zaplať příspěvek 2000,-“
- „Nákup materiálu na opravu. Zaplať 100,-“
- „Výhra v loterii 1000,-“
- „Obdržíš dotaci 2000,-“
- „Z banky obdržíš přeplatek 3000,-“

Karty Náhoda:

- „Jdi o 3 pole zpět.“
- „Získáváš kartu *Zrušen distanc*. Ponech si ji, dokud ji nepoužiješ.“
- „Postoupíš na nejbližší pole Trenér. Pokud projdeš startem, obdržíš 4000,-“
- „Zdržíš se na 2 kola.“
- „Jdi na Distanc (bez 4000,-).“
- „Zpět na nejbližší pole Finance.“
- „Zpět na poslední políčko ve hře (kůň Napoli). Obdržíš 4000,-“
- „Zpět na pole Distanc. Obdržíš 4000,- pokud jsi cestou zpět prošel startem.“
- „Zpět na nejbližší pole Finance.“
- „Zpět na Start. Obdržíš 4000,-“
- „Zpět na Start (bez 4000,-).“
- „Zdržíš se na 2 kola.“
- „Zdržíš se na 1 kolo.“
- „Zpět na Parkoviště. Obdržíš 4000,- pokud jsi cestou zpět prošel startem.“

## Peníze, dluhy a bankrot

Peníze se platí mezi hráči a bankou. Hráč se může dostat do dluhu:

- Hráč na tahu nemůže ukončit tah v dluhu. Prodá bance pole či dostihy,
  obchoduje, nebo nechá prodávat hru.
- Kdokoli jiný, kdo je po hodu v dluhu (třeba po kartě s narozeninami),
  prodává hned a sám.
- Když prodává hra: nejdřív dostihy, od nejlevnějších koní, pak pole od
  nejlevnějšího. Pole se bance prodávají za svou cenu.
- Kdo ani tak nezaplatí, zbankrotuje a končí ve hře. Jeho pole se vracejí
  bance a přicházejí o dostihy, jeho karta *Zrušen distanc* jde zpět do
  balíčku a jeho nabídky obchodu i nabídky jemu padají.

Hráč na tahu smí kdykoli během svého tahu prodat bance svá pole (jen ze
stájí bez dostihů) a dostihy.

## Obchod

Po pohybu smí hráč na tahu nabídnout obchod komukoli z ostatních: libovolné
své karty za libovolné karty druhého hráče a k tomu peníze jedním či druhým
směrem – nebo jen peníze, když hráči spolupracují. Obchodovat smí i hráč
v dluhu, pokud ten, kdo platí, peníze má.

Nabídka platí a nikoho nezdržuje:

- Druhý hráč ji smí kdykoli přijmout, i během tahů ostatních – pokud ten,
  kdo platí, v tu chvíli peníze má. Odmítat se nemusí; nabídky prostě
  zůstanou ležet.
- Kdo nabídku dal, smí ji kdykoli stáhnout.
- Každý hráč má nejvýš tři nabídky zároveň. Nabídka stejných karet stejnému
  hráči nahradí tu starou (třeba s lepší cenou).
- Nabídka, která už nejde splnit, hned padá: karta z ní změnila majitele,
  nebo stáj z ní dostala dostihy a není v obchodě celá. Nedostatek peněz
  nabídku jen pozdrží.

## Domácí pravidla

Zapínají se na obrazovce nové hry.

- **Pokuty na parkoviště** (zapnuto): pokuty – veterinář a karty Finance,
  kterými se platí bance – se hromadí uprostřed hracího plánu a kdo zastaví
  na Parkovišti, bere je. Podle původních pravidel je Parkoviště neutrální.
- **Koupě před hodem** (vypnuto): na začátku svého tahu smí hráč koupit
  volné pole, na kterém stojí – takové, které nekoupil, nebo které se mezitím
  vrátilo bance.

## Počítač

Počítač si drží rezervu 2 500 a hraje podle těchto pravidel:

- Koupí pole a tolik dostihů, kolik může, když mu rezerva zůstane.
- Na distancu použije kartu *Zrušen distanc*, když ji má.
- V dluhu nechá prodávat hru.
- Sbírá stáje, ze kterých má aspoň polovinu: když chybějící koně patří
  jednomu hráči, nabídne mu za ně peníze. Jinému počítači rovnou 3× jejich
  hodnotu, člověku 2× a každý tah o polovinu hodnoty víc, až do 3×. Na jednu
  nabídku nikdy nedá víc než polovinu peněz nad rezervou a nabídky stáhne,
  když by mu už stáj nedoplnily.
- Na nabídky jemu odpoví hned – přijme, nebo odmítne. Karty bere za jejich
  hodnotu (1,5× když mu doplní stáj) a své dává za 2× jejich hodnotu, 2,5×
  za koně ze stáje, kterou sbírá, a 3×, když by tím rozbil svou celou stáj
  nebo doplnil stáj toho, kdo nabízí. Rezervu si přitom drží.
- *Zrušen distanc* má pro něj cenu dostihů, o které by přišel, kdyby čekal
  na distancu: podle toho, jak pravděpodobně ostatní figurky jedním hodem
  zastaví na jeho koních s dostihy, a kolik tahů průměrně trvá dostat se
  z distancu. Bez dostihů je karta pro něj bezcenná.

Hodnota pole je jeho cena a k tomu zaplacené dostihy.

## Hrací plán

Po směru hodinových ručiček od startu. Poplatky jsou, kolik platí hráč za
zastavení na cizím koni s tolika dostihy; u veterinárního vyšetření je ve
sloupci Cena jeho poplatek.

| # | Pole | Stáj | Cena | Dostih stojí | Prohlídka | 1 dostih | 2 dostihy | 3 dostihy | 4 dostihy | Hlavní dostih |
|--:|---|---|--:|--:|--:|--:|--:|--:|--:|--:|
| 0 | Start | |  | | | | | | | |
| 1 | Fantome | oranžová | 1 200 | 1 000 | 40 | 200 | 600 | 1 800 | 3 200 | 5 000 |
| 2 | Finance | |  | | | | | | | |
| 3 | Gavora | oranžová | 1 200 | 1 000 | 40 | 200 | 600 | 1 800 | 3 200 | 5 000 |
| 4 | Veterinární vyšetření | | 500 | | | | | | | |
| 5 | 1. Trenér | | 4 000 | | | | | | | |
| 6 | Lady Anne | hnědá | 2 000 | 1 000 | 120 | 600 | 1 800 | 5 400 | 8 000 | 11 000 |
| 7 | Náhoda | |  | | | | | | | |
| 8 | Pasek | hnědá | 2 000 | 1 000 | 120 | 600 | 1 800 | 5 400 | 8 000 | 11 000 |
| 9 | Koran | hnědá | 2 400 | 1 000 | 160 | 800 | 2 000 | 6 000 | 9 000 | 12 000 |
| 10 | Distanc | |  | | | | | | | |
| 11 | Neklan | světle modrá | 2 800 | 2 000 | 200 | 1 000 | 3 000 | 9 000 | 12 500 | 15 000 |
| 12 | Přeprava | | 3 000 | | | | | | | |
| 13 | Portlancl | světle modrá | 2 800 | 2 000 | 200 | 1 000 | 3 000 | 9 000 | 12 500 | 15 000 |
| 14 | Japan | světle modrá | 2 800 | 2 000 | 240 | 1 200 | 3 600 | 10 000 | 14 000 | 18 000 |
| 15 | 2. Trenér | | 4 000 | | | | | | | |
| 16 | Kostrava | zelená | 3 600 | 2 000 | 280 | 1 400 | 4 000 | 11 000 | 15 000 | 19 000 |
| 17 | Finance | |  | | | | | | | |
| 18 | Lukava | zelená | 3 600 | 2 000 | 280 | 1 400 | 4 000 | 11 000 | 15 000 | 19 000 |
| 19 | Melák | zelená | 4 000 | 2 000 | 320 | 1 600 | 4 400 | 12 000 | 16 000 | 20 000 |
| 20 | Parkoviště | |  | | | | | | | |
| 21 | Grifel | růžová | 4 400 | 3 000 | 360 | 1 800 | 5 000 | 14 000 | 17 000 | 21 000 |
| 22 | Náhoda | |  | | | | | | | |
| 23 | Mohyla | růžová | 4 400 | 3 000 | 360 | 1 800 | 5 000 | 14 000 | 17 000 | 21 000 |
| 24 | Metál | růžová | 4 800 | 3 000 | 400 | 2 000 | 6 000 | 15 000 | 18 000 | 22 000 |
| 25 | 3. Trenér | | 4 000 | | | | | | | |
| 26 | Tara | žlutá | 5 200 | 3 000 | 440 | 2 200 | 6 600 | 16 000 | 19 500 | 23 000 |
| 27 | Furioso | žlutá | 5 200 | 3 000 | 440 | 2 200 | 6 600 | 16 000 | 19 500 | 23 000 |
| 28 | Stáje | | 3 000 | | | | | | | |
| 29 | Genius | žlutá | 5 600 | 3 000 | 580 | 2 400 | 7 200 | 17 000 | 20 500 | 24 000 |
| 30 | Podezření z dopingu | |  | | | | | | | |
| 31 | Shagga | tmavě zelená | 6 000 | 4 000 | 500 | 2 600 | 7 800 | 18 000 | 22 000 | 25 500 |
| 32 | Dahoman | tmavě zelená | 6 000 | 4 000 | 500 | 2 600 | 7 800 | 18 000 | 22 000 | 25 500 |
| 33 | Finance | |  | | | | | | | |
| 34 | Gira | tmavě zelená | 6 400 | 4 000 | 560 | 3 000 | 9 000 | 20 000 | 24 000 | 28 000 |
| 35 | 4. Trenér | | 4 000 | | | | | | | |
| 36 | Náhoda | |  | | | | | | | |
| 37 | Narcius | fialová | 7 000 | 4 000 | 700 | 3 500 | 10 000 | 22 000 | 26 000 | 30 000 |
| 38 | Veterinární vyšetření | | 1 000 | | | | | | | |
| 39 | Napoli | fialová | 8 000 | 4 000 | 1 000 | 4 000 | 12 000 | 28 000 | 34 000 | 40 000 |
