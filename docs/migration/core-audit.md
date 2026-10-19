# Audyt rdzenia przed migracją do Rust

Stan źródeł: 19 października 2026. Pierwsze wydanie obejmuje równorzędnie Linux i Windows. Audyt odczytuje implementację i istniejące testy; nie stanowi potwierdzenia uruchomienia tych testów. Punkty wymagające doprecyzowania są oddzielone od zachowania, które użytkownik już wykorzystuje.

## Źródła prawdy i granica zgodności

Format projektu opisuje `shared/model.ts`, wiązania i zależności `shared/bindings.ts` oraz `shared/graph.ts`, operatory `shared/operators.ts`. Faktyczne wykonanie znajduje się w `api/runner/index.ts` i `api/runner/control.ts`. Import z przeglądarki obsługuje `app/features/workspace/storage.ts`. `shared/CONTRACT.md` pomaga w orientacji, ale sam nie wystarcza jako specyfikacja produkcyjna.

Należy zachować istniejące projekty, konfiguracje komponentów i sens wyników. Nie należy kopiować błędów, przypadkowej kolejności słowników JS ani ograniczeń przeglądarkowego zapisu jako architektury aplikacji natywnej. Każda świadoma zmiana semantyki musi mieć decyzję, migrację i test porównawczy.

## Macierz funkcji i zgodności

| Obszar               | Działające zachowanie do przeniesienia                                                                   | Kryterium odbioru w Rust                                                                         |
| -------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Dokument             | `Project.version = 1`; scenariusze, komponenty, kontrakty, środowisko; pozycje węzłów                    | Import rzeczywistych plików v1 bez utraty konfiguracji; ponowny eksport zachowuje sens dokumentu |
| Węzły                | `trigger`, `if`, `request`, `select`, `build`, `formula`, `assert`, `condition`, `component`, `variable` | Każdy wariant ma jawny typ konfiguracji, walidator, porty i wykonawcę                            |
| Argumenty            | Literał JSON, referencja węzeł/port/ścieżka, parametr, środowisko, zagnieżdżone wyrażenie                | Argument może być dowolnym JSON; brak specjalnego uprzywilejowania ID                            |
| Połączenia danych    | Wyliczane z wszystkich liści referencji, także wewnątrz wyrażeń                                          | Jeden zapis źródłowy: wiązania; brak drugiej rozjeżdżającej się listy krawędzi danych            |
| Połączenia sterujące | `Graph.edges`: źródło, cel, opcjonalna gałąź `true`/`false`                                              | Cykl wykrywany łącznie dla zależności sterowania i danych                                        |
| Start                | Ręczny wyzwalacz; maksymalnie jeden na graf, bez wejść                                                   | Wykonanie wyłącznie aktywnej ścieżki; odłączony request nie wysyła HTTP                          |
| IF                   | Te same podstawowe predykaty co porównanie; wynik `false` nadal `passed`                                 | Wykonuje się tylko wybrana gałąź; pozostałe kroki dostają `skipped`                              |
| Porównanie           | Jeden blok `assert`, predykat typu w tym samym bloku                                                     | Obsługa typu, wartości, tolerancji i brakujących pól bez osobnego bloku typu                     |
| Warunek historyczny  | `condition` zwraca `blocked` przy niespełnieniu; dodatkowo lista predykatów `all`/`any`                  | Zgodność importu; UI odróżnia warunek blokujący od IF                                            |
| Request              | HTTP/HTTPS; argumenty URL, query, headers, JSON body; symulacja i wykonanie rzeczywiste                  | Testy lokalnego serwera potwierdzają metodę, kodowanie, treść, kolejność i brak ponowień         |
| Odpowiedź            | Porty `body`, `status`, `headers`, `duration`; tekst jeśli odpowiedź nie jest JSON                       | 4xx/5xx pozostają danymi do IF/asercji, błąd transportu jest `error`                             |
| Zmienna              | `inputs.value` → `outputs.value`, z zachowaniem typu; przypisanie pola odpowiedzi to referencja          | Kolejny przebieg pobiera świeże pole, a nie zapamiętaną wartość z edytora                        |
| Wybór pola           | Ścieżki obiektów/tablic, klucze cytowane, Unicode, pusta ścieżka = cały dokument                         | Testy `null`, `false`, `0`, pustego tekstu, pustych kluczy i kluczy z kropką                     |
| Budowanie danych     | Nazwane wejścia tworzą obiekt lub tablicę                                                                | Kolejność elementów tablicy jest jawnie określona, nie zależy od mapy Rust                       |
| Operatory            | Jeden katalog steruje portami, formularzem, walidacją i interpreterem                                    | Wszystkie 37 operatorów mają tabele przypadków i błędów                                          |
| Komponenty           | Definicje przypięte do `(id, revision)`; własne parametry i lokalne wyjścia                              | Zmiana rewizji nie zmienia istniejących instancji; odrzucana rekurencja                          |
| Granica komponentu   | Typowane porty, domyślne wartości, binding wyjścia także literal/parametr/env/wyrażenie                  | Ekstrakcja zachowuje każde zagnieżdżone wiązanie i sterowanie IF                                 |
| Cleanup              | Osobny graf po głównym przebiegu, także po błędzie/anulowaniu                                            | Usuwa utworzony zasób również po późniejszym błędzie; nie usuwa zasobu bez ID                    |
| Diagnostyka          | Całość, do wybranego kroku, pojedynczy krok z historycznymi wyjściami                                    | Wynik diagnostyczny jawnie oznacza historyczne dane i nie udaje pełnego przebiegu                |
| Raport               | Statusy kroku, faza, wejścia/wyjścia, request, actual/expected, zagnieżdżone wyniki                      | Zdarzenia i końcowy wynik dają zgodny status, dane są zredagowane przed publikacją               |
| Edycja               | Kopia konfiguracji, wielokrotne wklejenie, remap referencji wewnętrznych, zachowanie zewnętrznych        | Kopie są niezależne; gałęzie IF zachowane; nie powstają dwa Starty                               |
| Schematy danych      | Kontrakt jest obecnie ID + przykładowy JSON, a nie pełny JSON Schema                                     | Import nie obiecuje walidacji schematu, której stary silnik nie wykonywał                        |

Katalog operatorów: `add`, `subtract`, `multiply`, `divide`, `modulo`, `random`, `round`, `equals`, `notEquals`, `greater`, `less`, `atLeast`, `atMost`, `and`, `or`, `not`, `concat`, `letter`, `length`, `contains`, `abs`, `floor`, `ceiling`, `sqrt`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `ln`, `log`, `exp`, `power10`, `typeOf`, `exists`, `field`.

## Dokładna semantyka wykonania

### Wartości i wyrażenia

Brak pola jest obecnie reprezentowany przez JS `undefined`, a JSON `null` jest rzeczywistą wartością. Rust musi użyć osobnego typu, np. `ResolvedValue::{Missing, Present(Value)}`; `Option<Value>` też może poprawnie odróżnić `None` od `Some(Value::Null)`. Indeksowanie `serde_json::Value` zwracające zastępcze `Null` nie może być mechanizmem rozwiązywania ścieżek.

`exists` jest prawdziwe dla `null`, `false`, `0` i `""`; fałszywe tylko dla Missing. `notEmpty` odrzuca Missing, null, pusty tekst i pusty obiekt/tablicę; dopuszcza `false` i `0`. Porównania wartości są głębokie i bez konwersji typów. Typ rozróżnia null, tablicę i obiekt. Operatory liczbowe oraz logiczne nie konwertują tekstów, null ani bool. Wynik nieskończony jest błędem. Domyślne wartości portu zastępują Missing, nigdy null.

Wyrażenia są obliczane zachłannie; `and` i `or` obliczają obie strony. Referencja ukryta w zagnieżdżeniu nadal tworzy zależność grafu. `field` może zwrócić Missing. `letter` jest numerowane od 1, indeks tablicy w ścieżce od 0; tekstowe `length` i `letter` liczą punkty kodowe Unicode, nie bajty UTF-8 ani grafemy.

### Kolejność i gałęzie

Operacje są obecnie wykonywane szeregowo, każda jest `await`owana. Węzeł musi zaczekać na zakończenie wszystkich zależności danych oraz sterowania; przy kilku gotowych wybierana jest kolejność `graph.nodes`. To nie jest równoległy scheduler. Cztery równoczesne przebiegi dopuszczone przez API to osobna sprawa.

Graf ze Startem oblicza osiągalność po zależnościach, dopuszczając czyste argumenty `variable/select/build/formula` jako poprzedników. Odłączone requesty i komponenty nie powinny uruchamiać się tylko dlatego, że aktywna ścieżka chce ich danych. Graf bez Startu zachowuje legacy: wykonuje wszystkie korzenie.

IF wytwarza `passed: boolean` i zawsze ma status `passed`, chyba że samo obliczenie warunku zgłosi błąd. Krawędź wychodząca z IF wymaga `branch: "true" | "false"`; inne źródła nie mogą mieć tej właściwości. Niewybrana ścieżka propaguje `skipped`, nie błąd.

Scalenie alternatywnych krawędzi sterowania działa jak OR aktywacji, ale scheduler czeka na wszystkich poprzedników. Pominięta alternatywa nie blokuje aktywnej. Błąd aktywnego poprzednika nie zostaje ukryty przez powodzenie innego. Referencja danych do pominiętego węzła nadal jest niedostępną zależnością; OR dotyczy wyłącznie sterowania.

`assert` z wynikiem fałszywym daje `failed`, ale zachowuje wyjścia i może być poprzednikiem dalszych operacji przy `continue`. `condition` daje `blocked` i blokuje potomków. `error/blocked/skipped/cancelled` poprzednika zwykle blokują głównych potomków. `errorPolicy=stop` pomija pozostałą część głównego grafu po failed/error/blocked, bez pomijania cleanup.

### Komponenty, cleanup i diagnostyka

Komponent ma osobną mapę wyjść i argumenty zamiast parametrów scenariusza. Środowisko pozostaje wspólne. Status agreguje dzieci z priorytetem cancelled → error → blocked → failed. Nawet komponent zakończony błędem może wyeksportować już utworzone ID.

Cleanup startuje zawsze w `finally`, dostaje własny budżet 10 s, niezależny od anulowania głównego przebiegu, i korzysta z wyników głównej fazy. Brak referowanego pola zasobu pomija cleanup. Dostępny eksportowany zasób komponentu może być użyty mimo jego późniejszego błędu. Krawędź porządkowa w cleanup oznacza zasadniczo „po zakończeniu”, a nie „tylko po sukcesie”. IF i osiągalność Startu w cleanup wymagają osobnych testów, ponieważ ogólna aktywacja i wyjątki cleanup nakładają się.

„Do kroku” wybiera domknięcie wszystkich przodków zależności. „Tylko krok” ładuje wyniki główne wcześniejszego raportu i omija własną bramkę aktywacji; nie omija bramek zagnieżdżonego komponentu. Oba tryby nadal uruchamiają cleanup. Historia jest już zredagowana, więc nie gwarantuje odtworzenia tajnych argumentów; nie wolno traktować jej jako cache poprawnych danych wykonania.

Anulowanie użytkownika daje końcowe `cancelled`. Przekroczenie budżetu głównego 120 s daje błąd przebiegu. Nieudana asercja/błąd cleanup powoduje końcowe `failed`. Sam `skipped` nie oznacza niepowodzenia. Pojedyncza diagnostyka przechowuje opis użycia historii w polu `run.error`, ale nie uznaje samego tego opisu za błąd — w Rust potrzebne oddzielne pole ostrzeżenia/proweniencji.

### HTTP i ochrona danych

Base URL jest prefiksem obejmującym ścieżkę. Request `:argument` koduje segment; `{{argument}}` obsługuje tekst i fallback do środowiska. Cały token w JSON body zachowuje typ JSON; token osadzony w tekście produkuje tekst. Query jest kodowane przez API URL. Przekierowania są odrzucane, dane logowania w URL zabronione, brak automatycznych retry. GET/HEAD z body to błąd. Rust nie może niejawnie wprowadzić retry dla POST tworzącego koszyk.

Obecne limity: request domyślnie 10 s, maksymalnie 60 s; odpowiedź 2 MiB także strumieniowo, JSON odpowiedzi maksymalnie 100 poziomów; główny przebieg 120 s, cleanup 10 s, 1000 wykonanych kroków łącznie z dziećmi, raport 16 MiB. Są to punkty wyjścia konfiguracji produktu, a nie uzasadnione SLA. Obecny limit raportu liczy zagnieżdżone dane wielokrotnie; nowy licznik powinien mieć jedną precyzyjną definicję.

Silnik zachowuje surowe wartości wewnątrz, publikuje zredagowane kopie. Redakcja zna sekrety sesyjne, zadeklarowane nazwy oraz wykrywa konwencjonalne klucze (`authorization/cookie/password/secret/token/api-key`), credentiale URL, query tokeny i wartości Bearer. Maskuje też kodowanie URL/form. Obejmuje snapshot, wejścia, wyjścia, błędy i callbacki.

Wykrywanie sekretu dopiero w późniejszej odpowiedzi nie może cofnąć wycieku we wcześniejszym callbacku. Heurystyki nazw nie gwarantują ochrony dowolnego sekretu. Produkcyjna redakcja wymaga jawnej klasyfikacji sekretów przed publikacją, ograniczenia surowych logów oraz testów negatywnych wszystkich kanałów, w tym `tracing`, crashów i zapisu na dysku.

## Import v1 i trwałość

1. Obsłużyć plik pojedynczego projektu oraz wrapper workspace `{version:1, activeId, projects}`. Workspace wymaga niepustej listy, unikalnych ID projektów i istniejącego `activeId`.
2. Czytać istniejące camelCase i tagi bindingów dokładnie; zachować rewizje, kolejność węzłów, pozycje, `templateInputs`, zagnieżdżone referencje, gałęzie i dowolne literały JSON.
3. Rozdzielić DTO v1 od zwalidowanego modelu domenowego. Nowa konfiguracja typu enum nie powinna narzucać zmiany starego wire format.
4. Przeprowadzić walidację strukturalną i semantyczną przed podmianą otwartego dokumentu. Odrzucenie importu nie zmienia bieżącego projektu ani autosave. Błędy powinny zawierać ścieżkę JSON i ID węzła.
5. Zachować brak Startu jako jawny tryb zgodności. Nie wstawiać automatycznie Startu, który zmieni liczbę requestów. Konwersja na nowy przepływ musi być osobną edycją z undo.
6. Aktualny importer przenosi wyłącznie historyczne demo `localhost/127.0.0.1:3000/demo/(shop|support)` na port 4318. W aplikacji natywnej endpoint demo może nie istnieć; zastosować dedykowaną migrację demonstracji, a nie przepisywać adresy własnych API.
7. `publicProject` usuwa wartości wyłącznie nazw z `environment.secretNames`; zwykły projekt może nadal zawierać literały z credentialami. Zdefiniować produkcyjny eksport publiczny osobno od lokalnego zapisu. Sekrety sesyjne nie trafiają do schowka ani pliku projektu.
8. Przeglądarkowego `localStorage` aplikacja natywna sama nie odczyta. Dostarczyć eksport ze starej aplikacji i import w nowej; oryginalny plik pozostawić nietknięty, zapisać kopię przed migracją wersji.

Obecne limity importu: do 100 scenariuszy, 200 definicji komponentów, 200 kontraktów, 200 węzłów i 1000 krawędzi na graf, 100 wejść/portów, do 12 poziomów i 200 elementów jednego bindingu, tekst do 100000 jednostek JS. Dwie istniejące walidacje mają różne reguły głębokości JSON; zunifikować je w jeden walidator limitów. `serde` nie może po cichu odrzucać nieznanych pól konfiguracji importu i eksportować niepełnego dokumentu; albo zachować rozszerzenia, albo zgłosić ostrzeżenie/odmowę z pełną ścieżką.

## Proponowane granice crate'ów

| Crate / moduł          | Odpowiedzialność                                                                                                                                               | Nie powinien zależeć od             |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| `apiforge-core`        | Domena: ID, bindingi, typowane konfiguracje, porty, statusy, błędy, ścieżki JSON, czyste operatory; DTO v1, migracje i walidacja w wydzielonym module `format` | egui, Tokio, HTTP, pliki            |
| `apiforge-runner`      | Kompilacja DAG, aktywacja IF, wykonanie, komponenty, cleanup, limity, zdarzenia, anulowanie; adapter HTTP jako wydzielony moduł                                | egui i globalny stan interfejsu     |
| `apiforge-storage`     | Atomowy zapis dokumentu, kopie, historia, adapter magazynu sekretów                                                                                            | egui jako format domeny             |
| `apiforge-application` | Komendy edytora i undo/redo; sesja, koordynacja runnera i storage, publiczny stan dla UI                                                                       | egui i szczegóły renderera          |
| `apiforge-desktop`     | egui, graf, formularze, schowek, raport i integracja okien Linux/Windows                                                                                       | wewnętrzne szczegóły sieci/sekretów |

CLI/headless może być osobnym binarnym frontendem korzystającym z tych samych application/runner, gdy zostanie uwzględniony w zakresie wydania. Nie potrzeba portować lokalnego serwera Express: natywny frontend komunikuje się z asynchronicznym runnerem Tokio i transportem reqwest przez kanały wewnętrzne.
Można początkowo połączyć małe adaptery w moduły, zachowując kierunek zależności i publiczne kontrakty. Produkcyjność nie wynika z liczby crate'ów. Silnik powinien przyjmować zwalidowany snapshot planu oraz `RunContext` z wstrzykniętym transportem, zegarem i RNG; emitować typowane zdarzenia przez ograniczony kanał. UI nie może wykonywać blokującego I/O w klatce egui.

## Kolejność implementacji i konkretne testy odbioru

1. **Zamrożenie kontraktu.** Wyeksportować fixtures istniejących projektów: sklep, zgłoszenia, operatory, workflow koszyka, komponenty z rewizjami i kopie. Dodać expected trace z ID, statusem, aktywną gałęzią i wynikami; pominąć losowe ID i czasy. Node pozostaje referencją porównawczą do chwili migracji.
2. **Domena i import.** Implementować DTO, walidację, ścieżki oraz operatory. Wspólny korpus uruchamia TS i Rust. Sprawdzić Missing/null/false/0/puste klucze, liczby graniczne, Unicode, zagnieżdżone bindingi, brakującą rewizję, cykle mieszane i rekurencję. Fuzzing parsera/importera musi kończyć się kontrolowanym wynikiem, bez paniki i nieograniczonej alokacji.
3. **Planner i sterowanie.** Odwrócona kolejność zapisu węzłów nadal daje ceny → koszyk → dodanie, maksymalnie jeden request w locie. IF true/false wysyła wyłącznie żądania aktywnej gałęzi. Testować złączenia, błąd jednej aktywnej alternatywy, nieaktywną referencję oraz odłączony HTTP współdzielący czysty argument. Zachować graf legacy bez Startu.
4. **HTTP i cleanup.** Lokalny fixture sprawdza typed body i URL, 4xx jako dane, odpowiedź tekstową, timeout, anulowanie podczas czytania, zawyżony/zaniżony/brak content-length, limit streamu i przekierowanie. Scenariusz koszyka musi utworzyć, dodać po zakończeniu utworzenia, porównać sumę i faktycznie usunąć zasób także po błędzie asercji.
5. **Komponenty i diagnostyka.** Testować pinned rewizje, wyjście ID po późniejszym błędzie dziecka, limit głębokości, częściowe wykonanie i pochodzenie historycznych wartości. Zagnieżdżony IF działa również przy pojedynczym uruchomieniu rodzica.
6. **Granica bezpieczeństwa.** Kanoniczne i zakodowane sekrety nie występują w zdarzeniach, raportach, `tracing`, eksporcie ani plikach crash. Debug danych wrażliwych ma domyślnie ukryty tekst. Zredagowany raport nigdy nie jest źródłem prawdziwych sekretów kolejnego przebiegu.
7. **Komendy edytora.** Operacje kopiuj/wklej/duplikuj/usuń/połącz/ekstrakcja komponentu jako atomowe komendy. Testy niezależności wielokrotnej kopii, remapowania wyłącznie wewnętrznych liści, zachowania rewizji/gałęzi oraz undo/redo przy zmianie dokumentu.
8. **Trwałość i release.** W planie nadrzędnym wybrano SQLite: transakcje, migracje schematu i spójny backup obejmujący aktywny WAL. Eksport plikowy korzysta z pliku tymczasowego i bezpiecznej podmiany. Testować przerwanie zapisu, uszkodzone dane i odzyskanie kopii. Test integracyjny uruchamia ten sam projekt przez GUI i bezpośrednio przez headless runner. CI i odbiór obejmują Linux oraz Windows od pierwszego wydania: zapis/odzyskiwanie, sekrety sesyjne (keyring tylko jeśli dojdzie jako osobna funkcja), schowek, TLS/proxy, anulowanie oraz natywne paczki instalacyjne. Testować semantykę atomowej podmiany na obu systemach, a nie zakładać przenośności operacji plikowych Unix. Pipeline wymaga formatowania, clippy, testów, skanowania zależności i podpisanych/reprodukowalnych artefaktów zgodnie z ustaloną platformą docelową.

Istniejące zestawy do odwzorowania: `tests/runner.test.ts`, `tests/workflow.test.ts`, `tests/operators.test.ts`, `tests/variables.test.ts`, `tests/project.test.ts`, `tests/runner-api.test.ts` oraz testy operacji grafu, ekstrakcji komponentów, ścieżek JSON i redakcji pod `app/features/`. Nie wystarczy przepisać nazw testów: krytyczne są zaobserwowane żądania fixture i brak niedozwolonych efektów ubocznych.

## Decyzje i słabości wymagające zamknięcia przed wydaniem

- **Osiągalność po danych.** Obecny planner chodzi po następcach zależności danych i sterowania, a czyste argumenty dołącza wstecz. Współdzielony czysty producent może więc rozszerzyć osiągalność także na inny jego konsument HTTP. W produkcyjnym modelu jawnie oddzielić aktywację sterującą od potrzeby danych; dodać test, zanim uzna się regułę „odłączony request nigdy nie rusza” za spełnioną.
- **Złączenia i porządek.** OR aktywacji nie oznacza „pierwszy zakończony wygrywa”; teraz czekamy na wszystkich. Ustalić jawnie złączenie alternatywne oraz ewentualny przyszły join AND. Zachować seryjność jako domyślne zachowanie; równoległość wymaga osobnego jawnego bloku/ustawienia i testów efektów ubocznych.
- **Wartości liczbowe.** JS używa IEEE-754; Rust/serde może rozróżniać integer i float inaczej niż stare equality. Ustalić zgodność `1`/`1.0`, bezpieczny zakres integer, zaokrąglanie liczb ujemnych, `-0`, modulo i zapis wykładniczy. Ceny pieniężne potrzebują jawnej decyzji decimal/minor units, bez niejawnej zmiany starych formuł.
- **Stringi i tablice.** `concat` używa JS `String` także dla obiektów/tablic, a `build` tablicy używa JS porządku kluczy. Rust potrzebuje kompatybilnego formatowania lub migracji. Limit 100000 „znaków” JS liczy UTF-16, nie bajty UTF-8; nazwać jednostkę limitów.
- **Niespójne domyślne argumenty.** Formula węzła uzupełnia porty domyślne przed wykonaniem; bezpośrednie wyrażenie z brakującym argumentem błędnieje. `typeOf` ma wymagany argument, więc mimo pomocniczego `jsonType(Missing)` nie pozwala obecnie obserwować Missing. Ustalić świadome reguły i diagnostykę, zamiast przypadkiem zmienić je podczas portu.
- **RNG i powtarzalność.** `random` używa `Math.random`; testy i replay powinny przyjmować RNG/seed, a raport zapisywać seed. Nie wprowadzać niejawnej deterministyczności do importu bez zdefiniowania oczekiwań.
- **Historia i autoryzacja diagnostyki.** Poprzedni raport jest walidowany głównie przez scenariusz/tryb, bez fingerprintu projektu. Edycja URL albo rewizji może połączyć dane ze starego planu z nowym. Wymaga snapshot hash i jawnej proweniencji. Cleanup po diagnostyce może skasować historyczny zasób — potrzebna celowa polityka odróżniająca zasoby utworzone w bieżącym wykonaniu od odziedziczonych.
- **Redakcja nie jest walidacją danych.** Nie zamieniać sekretów na tekst w modelu roboczym, bo zmieni to działanie requestów i typy. Osobno zabezpieczyć zapis, debug oraz eksport; nazwy kluczy nie stanowią kompletnego systemu klasyfikacji.
- **Sieć desktopowa.** Aplikacja do testowania API musi obsługiwać także localhost i sieci prywatne. Nie kopiować globalnego zakazu tych adresów jako ochrony SSRF. Jednocześnie ustalić politykę importowanego niezaufanego projektu, proxy, TLS/custom CA, uwierzytelnienia, cookies i transferu credentiali. Obecnie brak jawnego cookie jar i retry; dodanie ich zmienia testy.
- **Historia i przerwanie procesu.** API przechowuje wyłącznie sesyjny raport (do 100/32 MiB), bez trwałości i recovery. Cleanup nie może wykonać się po SIGKILL/awarii OS. Produkcja potrzebuje trwałego dziennika utworzonych zasobów i kontrolowanego wznowienia sprzątania; obietnica „zawsze sprząta” byłaby nieprawdziwa.
- **Spójność walidacji.** Obecne walidatory runnera i projektu częściowo dublują reguły. Błędy są stringami, `NodeConfig` ma luźny worek opcji i możliwe pola nieużywane dla danego rodzaju. Rust powinien walidować raz do typowanego modelu i zwracać stabilne kody błędów z kontekstem, bez `unwrap` na niezaufanych dokumentach.
- **Skala i responsywność.** Limity 200 węzłów/graf i 1000 kroków nie zastępują pomiarów. Ustalić budżet klatki, pamięci i czas otwierania dużego dokumentu; kolejka zdarzeń musi mieć backpressure, a wirtualizacja raportu nie może trzymać wielu pełnych kopii odpowiedzi na każdą klatkę.
