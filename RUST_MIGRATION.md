# API Forge: migracja do Rust + egui

Status: rozpoczęta implementacja R0/R1, 19.10.2026. Powstały `apiforge-core` (import v1, DTO, walidacja) i `apiforge-desktop` (natywny podgląd grafu), deterministyczny korpus eksportów oraz CI Linux/Windows. Edycja, runner i SQLite pozostają do przeniesienia. Wydanie produkcyjne nie jest jeszcze gotowe. Ten dokument zastępuje założenia technologiczne wcześniejszych planów PoC; aplikacja TypeScript pozostaje wersją referencyjną.

## Cel i ustalenia

Użytkownik wybrał **Linux i Windows od pierwszego wydania**. Przenosimy cały obecny sposób pracy: budowanie testów blokami, Start, IF / ELSE, kolejność requestów, zmienne z odpowiedzi, zagnieżdżone operatory, własne komponenty i ich rewizje, kopiowanie konfiguracji, raport i sprzątanie. Silnik pozostaje niezależny od domeny; argumenty nie mają specjalnego traktowania nazw `id` ani nazw endpointów.

Wydanie oznacza sprawdzony pakiet desktopowy na obu platformach, import dotychczasowych projektów, trwały zapis z odzyskiwaniem oraz udokumentowane zachowanie błędów. Samo uruchomienie okna i kilka klocków nie zamyka żadnego etapu migracji funkcjonalnej.

Zakładamy x86_64. Dokładne minimalne wydania Windows i dystrybucje Linuksa zostaną zapisane w R0 po sprawdzeniu wymagań bibliotek i pakowania; nie deklarujemy obsługi systemu na podstawie samego cross-compilation. macOS, ARM, synchronizacja chmurowa, pluginy, pętle, harmonogramy i automatyczna aktualizacja nie należą do pierwszego wydania. Instalacja nowszej wersji musi jednak zachować dane i umożliwiać odzyskanie kopii bazy.

## Zespół i odpowiedzialność

| Rola                | Własność                                                                | Wynik obecnego zlecenia                                 |
| ------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------- |
| Koordynator         | decyzje, granice modułów, kontrakty, zależności, integracja, CI, odbiór | ten plan i kolejność prac                               |
| Agent silnika       | zachowanie modelu, importu, operatorów i wykonania                      | [audyt core](docs/migration/core-audit.md)              |
| Agent interfejsu    | interakcje egui, edytor, operacje użytkownika i raport                  | [audyt UI](docs/migration/ui-audit.md)                  |
| Agent niezawodności | zapis, sekrety, transport, testy wydania i dystrybucja                  | [audyt produkcyjny](docs/migration/production-audit.md) |

Subagenci zostali uruchomieni równolegle. Implementacja po ustaleniu wspólnych kontraktów również przebiega równolegle, w rozłącznych katalogach. Jeden właściciel każdego pliku; pliki Cargo, lockfile i kontrakty wspólne zmienia koordynator. Zmiana zachowania wymaga testu i aktualizacji specyfikacji. Nie tworzymy osobnych modeli projektu dla UI i runnera.

## Architektura docelowa

Jedna aplikacja natywna. egui obsługuje UI, a asynchroniczny runner działa poza wątkiem rysowania. Express, przeglądarka, lokalny serwer sterujący i Node nie są zależnościami uruchomieniowymi nowego wydania. Narzędzia TypeScript pozostają czasowo wyłącznie do porównań zgodności.

```text
crates/
  apiforge-core/          model domeny, walidacja, graf, operatory, format v1
  apiforge-runner/        harmonogram, HTTP, symulacja, anulowanie, cleanup
  apiforge-storage/       SQLite, migracje, kopie, import/eksport
  apiforge-application/   komendy edycji, historia, stan sesji, koordynacja usług
  apiforge-desktop/       eframe/egui, canvas, inspector, dialogi, raport
fixtures/compatibility/   projekty v1 i oczekiwane wyniki semantyczne
```

Core nie importuje egui, reqwest, SQLite ani logiki okna. Runner i storage zależą od core, ale nie od siebie. Application zależy od core i ustalonych interfejsów usług; desktop jest miejscem ich połączenia. Współdzielony model raportu i zdarzeń należy do core. Konkretna organizacja traitów wynika z potrzeb testowania; nie tworzymy warstw dla samej abstrakcji.

Model pliku jest oddzielony od zwalidowanego modelu wykonania. `NodeConfig` nie będzie jednym workiem opcji dla każdego rodzaju klocka: typowane warianty `Node`, `Binding`, `Predicate`, `Operator`, `StepStatus`, nowe typy identyfikatorów i osobny wynik walidacji. DTO formatu v1 zachowuje istniejące nazwy i zapis; konwersja do modelu typowanego zwraca diagnostykę z lokalizacją błędu.

Stan trwały, stan edytora i stan wykonania są oddzielne. Każda komenda edycji przechodzi przez application; rysowanie nie mutuje projektu bokiem. Cofanie i ponawianie obejmują atomowe operacje: wklejenie grupy, zmianę połączeń i zapis rewizji. Wklejenie używa zamrożonego fragmentu, nowych ID i przepina wewnętrzne referencje także w drzewach operatorów.

UI wysyła komendy Start/Cancel do usługi runnera. Zdarzenia mają RunId, numer sekwencji i pełną ścieżkę instancji kroku wewnątrz komponentów. Kanały są ograniczone; nie wolno bez końca klonować raportów co klatkę ani zgubić końcowego statusu przy przeciążeniu. Wywołanie HTTP, zapis bazy, parsowanie dużych danych i migracja nie blokują rysowania. Zamknięcie okna podczas testu ma jawny przebieg anulowania i sprzątania.

## Biblioteki i wybór wersji

Docelowe składniki: egui/eframe, Tokio, reqwest, serde/serde_json, rusqlite/SQLite, typowane błędy i tracing bez sekretów. Format kolekcji musi zachowywać wymaganą kolejność; nie zamieniamy bez analizy map JS na sortujące `BTreeMap` ani losowo iterowane `HashMap`.

Sprawdzona dokumentacja pokazuje egui/eframe **0.37.0** i zmianę interfejsu `App`: wymagane jest `ui`, obok `logic`. Lokalne rustc to **1.99.0**, natomiast cache ma m.in. eframe/egui 0.36.2. To informacja o środowisku, nie decyzja wyboru starszej wersji. [egui](https://docs.rs/egui/latest/egui/), [eframe App](https://docs.rs/eframe/latest/eframe/trait.App.html).

W R0 koordynator ustala jeden zgodny zestaw egui, eframe, renderera, biblioteki grafu i testów. Kandydatem dla grafu jest [egui-snarl](https://docs.rs/egui-snarl/latest/egui_snarl/); decyzja wymaga weryfikacji kompatybilności i obsługi portów danych/sterowania, zagnieżdżonych pól, historii oraz dostępności. Graf biblioteki jest adapterem widoku, nigdy źródłem prawdy domeny. Jeśli biblioteka nie spełnia kryteriów, własny canvas dostaje osobne zadania interakcji, hit-testów i dostępności; nie ukrywamy kosztu pod hasłem „egui obsługuje grafy”.

Używamy przypiętego toolchainu, Cargo.lock i ograniczonych funkcji zależności. Rustdoc `latest` służy rozeznaniu; implementacja i CI korzystają z wybranego zestawu. Zależności i licencje podlegają kontroli w CI.

## Kontrakt zgodności

1. Import `Project.version = 1` i eksportów workspace z obecnej aplikacji; zachowanie ID, parametrów, referencji, komponentów/revisions, próbek i połączeń IF. Źródłowe pliki pozostają nietknięte. Aplikacja desktopowa nie odczyta automatycznie localStorage przeglądarki: dotychczasowy interfejs musi umożliwić eksport całego obszaru, a desktop jego import.
2. Import bez uruchamiania requestów. Sekrety nie są częścią migracji ani zwykłego eksportu. Błędny lub nowszy format daje komunikat i nie nadpisuje istniejących danych. Zmiana formatu ma jawny numer wersji, migrację, test i kopię sprzed zmiany.
3. `Missing` jest inną wartością niż JSON `null`. Ścieżki pól, tablic i kluczy zawierających kropki, nawiasy czy Unicode muszą działać po migracji.
4. Wszystkie obecne rodzaje bloków i 37 operatorów mają przeniesione testy. Liczby JS, zaokrąglenie, modulo, Unicode i kolejność pól wymagają testów granicznych; przejście na domyślne zachowanie Rusta nie jest automatycznie zgodne. Zmiana kontraktu wymaga świadomej migracji, nie ukrytej „poprawki”.
5. Każdy request jest oczekiwany przed następnym. Start, osiągalność, IF false jako poprawny wynik, pomijanie nieaktywnej ścieżki, wymagane dane przy scaleniu i polityka błędów mają osobne przypadki zgodności. Starszy graf bez Startu zachowuje jawnie opisany tryb legacy. To nie jest bezpośrednia kopia reguł n8n.
6. Niezmienne migawki wejść uruchomienia; zmiana projektu nie zmienia trwającego testu. Pełny przebieg, wykonanie do klocka i pojedyncza diagnostyka mają określone pochodzenie danych. Dane raportu po redakcji nie zastępują oryginalnych wartości w obliczeniach runnera.
7. Cleanup ma niezależny budżet i korzysta z już uzyskanych wyników także po błędzie komponentu. Awaria procesu nie daje gwarancji cleanup; po restarcie przebieg jest oznaczony jako przerwany, bez automatycznego ponawiania operacji biznesowych. Ręczne sprzątanie wymaga dostępnych danych i jawnego zlecenia użytkownika.
8. HTTP 4xx/5xx pozostaje odpowiedzią do sprawdzenia, nie błędem transportu. Bez niejawnych przekierowań, ponowień czy konwersji typów. Konfiguracja proxy/TLS jest jawna i testowana na obu platformach.

Istniejące testy są bazą przypadków, a nie dowodem poprawności każdej historycznej implementacji. Audyty wskazują rozbieżności; dla każdej zapisujemy decyzję: zgodność, naprawa błędu z regresją albo zmiana wersji. Zrzuty porównawcze normalizują tylko metadane niedeterministyczne (czas, RunId); nie wolno usuwać statusów, kolejności requestów ani wartości, żeby testy przeszły.

## Poprawki wymagane przy migracji

Audyt nie traktuje obecnego silnika jako bezbłędnego wzorca. Przed implementacją runnera zamykamy następujące decyzje w testach kontraktowych:

- Aktywacja ścieżki i pobieranie argumentów są odrębne. Dołączenie czystego producenta danych nie może samo zezwolić na HTTP u pozostałych jego konsumentów. Legitymne zależności istniejących projektów muszą mieć zdefiniowane odwzorowanie migracji, bez cichego poszerzenia ani zawężenia requestów.
- Pojedyncza diagnostyka nie może automatycznie sprzątać zasobów odziedziczonych z historii. Każde wyjście zasobu ma pochodzenie; standardowy cleanup dotyczy bieżącego wykonania. Sprzątanie historycznego zasobu jest osobną świadomą akcją.
- Historia używana do diagnostyki jest przypięta do fingerprintu projektu, środowiska i rewizji komponentów. Zredagowany tekst nie staje się prawdziwym tokenem następnego requestu.
- Domyślne argumenty operatora, obserwacja Missing, formatowanie obiektów do tekstu i liczby mają jedną specyfikację oraz zestaw testów. Nie powielamy niespójności dwóch dzisiejszych walidatorów.

Te korekty są obowiązkami R1/R2/R4, z konkretnymi regresjami opisanymi w audycie core. Nie są deklaracją, że błędy w aktualnej wersji zostały już naprawione.

## Plan wykonania i zależności

| ID  | Właściciel                 | Zakres i zależności                                                                                          | Warunek odbioru                                                                                                                         |
| --- | -------------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| R0  | koordynator + UI           | zestaw bibliotek, wsparcie OS, ADR, Cargo workspace, fmt/clippy/test/build CI; przed dalszą implementacją    | kompilacja wybranego stosu i test okna na Linux/Windows; zamknięty wybór canvas; udokumentowane granice modułów                         |
| R1  | silnik + koordynator       | DTO v1, model domeny, walidator, fixture eksportu; po R0                                                     | wszystkie obecne projekty demonstracyjne i negatywne importy; round-trip bez utraty danych; graniczne rozmiary i głębokości             |
| R2  | silnik                     | evaluator, ścieżki, kompilacja planu grafu, komponenty; po R1                                                | testy typów/operatorów/IF/zależności, deterministyczność, Missing/null i przypadki JS–Rust                                              |
| R3  | niezawodność               | magazyn, schemat DB, import atomowy, backup/recovery; po R1                                                  | brak utraty potwierdzonego zapisu przy kill/restart, pełnym dysku i nieudanej migracji; brak sekretów w DB/eksporcie                    |
| R4  | silnik                     | runner async, HTTP, symulacja, anulowanie, cleanup, zdarzenia; po R2                                         | fixture HTTP: ceny → koszyk → produkty, obie gałęzie, błędy i timeouty, brak równoległych requestów w jednej ścieżce; bounded resources |
| R5  | koordynator + UI           | komendy edycji, undo/redo, dokument/sesja, powiązanie ze storage; po R1, integracja R3                       | walidowane atomowe edycje, kopie konfiguracji bez aliasowania, dirty/saved/error, rollback transakcji                                   |
| R6  | UI                         | shell egui, biblioteka, canvas, porty, kontrola fokusu, keyboard; po R0/R1/R5                                | pełna obsługa dodawania, łączenia, zaznaczania, przesuwania, usuwania, kopiuj/wklej i undo na obu OS                                    |
| R7  | UI                         | inspector, zagnieżdżone argumenty, przeciąganie zmiennych, selektor response, komponenty; po R2/R5/R6        | odtworzenie dzisiejszych przepływów bez pisania kodu; konfiguracja typu przez Porównanie; jawne źródła wartości                         |
| R8  | UI + silnik                | raport, resize, błędy wewnętrzne, anulowanie, historia/eksport; po R3/R4/R6                                  | raport przypięty do właściwej migawki, płynny UI podczas HTTP, poprawne cleanup i diagnostyka, redakcja przed zapisem/prezentacją       |
| R9  | niezawodność + koordynator | testy natywne, wydajność, TLS/proxy, zabezpieczenia importu, pakowanie; przygotowanie od R0, odbiór po R3–R8 | wszystkie bramki wydania poniżej i artefakty z CI na obu OS                                                                             |
| R10 | koordynator                | migracja danych użytkownika, próba instalacji/upgrade/recovery, odbiór całego produktu                       | Linux i Windows spełniają ten sam zakres; instrukcja migracji i powrotu; dopiero wtedy Rust jest wersją domyślną                        |

Fala 1: R0 oraz zamrożenie kontraktów i fixtures. Fala 2: po R1 równolegle silnik R2/R4, magazyn R3 i UI/komendy R5/R6. Fala 3: R7/R8 i integracja. Fala 4: R9/R10. Wcześniejsze etapy mogą dawać wewnętrzne przyrosty robocze, lecz nie są wydaniem produkcyjnym z pominięciem pozostałych bramek.

Każde zadanie kończy się kodem, testami adekwatnymi do zachowania, krótką dokumentacją decyzji i listą pozostałych problemów. Agent nie oznacza całej migracji jako gotowej na podstawie kompilacji swojego modułu. Koordynator rozstrzyga konflikty semantyki, integruje zmiany i sprawdza cały przepływ użytkownika.

## Bramki wydania produkcyjnego

| Obszar             | Wymagany dowód                                                                                                                                                                                                                                                                                                                    |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Zgodność funkcji   | macierz audytów zamknięta; wszystkie obecne przepływy użytkowe przeniesione; każda świadoma rozbieżność opisana                                                                                                                                                                                                                   |
| Silnik             | testy lokalnego HTTP, kolejności, IF/scalenia, anulowania podczas connect/read/komponentu, stop/continue, cleanup, diagnostyki i stabilnej tożsamości kroków                                                                                                                                                                      |
| Dane               | testy round-trip v1 i workspace, uszkodzonego/nowego formatu, crash recovery, backup/restore, nieudanego importu/migracji i aktualizacji pakietu                                                                                                                                                                                  |
| Bezpieczeństwo     | limity parsowania przed kosztowną alokacją, brak wykonywania kodu z projektu, poprawna weryfikacja TLS, kontrola zależności/licencji, testowe sekrety nie trafiają do raportów/logów/snapshotów/DB                                                                                                                                |
| UI                 | egui_kittest dla komend i interakcji oraz natywne testy okna; fokus, schowek tekstowy vs blokowy, klawiatura, IME/Unicode, drag-and-drop, DPI i resize na Linux/Windows                                                                                                                                                           |
| Dostępność         | nazwy i fokus kontrolek, alternatywa klawiaturowa dla przewodów, ocena czytników ekranu na obu OS; custom canvas nie jest automatycznie dostępny dzięki egui                                                                                                                                                                      |
| Wydajność          | pomiary na zapisanej konfiguracji sprzętu i release buildzie: propozycja 200 klocków/1000 przewodów p95 <= 16,7 ms i syntetyczny profil 500/1000 p95 <= 33 ms podczas edycji (profil 500 nie zmienia limitu importu v1); brak blokady UI przez HTTP i ograniczony wzrost pamięci raportów; progi utrwalone po pomiarze bazowym R0 |
| Dystrybucja        | natywny CI Linux/Windows: fmt, clippy z warnings jako błędy, testy, release build; instalacja, start, upgrade i deinstalacja zachowująca dane; sumy kontrolne i manifest wersji/zależności; weryfikacja pochodzenia artefaktów                                                                                                    |
| Operacje i obsługa | logi diagnostyczne z rotacją, czytelne komunikaty awarii bez danych wrażliwych, instrukcja backup/restore i migracji; bez samoczynnego replay po restarcie                                                                                                                                                                        |

SQLite pracuje w transakcjach z wersjonowaniem schematu i jednym właścicielem połączenia zapisu poza UI; backup korzysta z bezpiecznego mechanizmu bazy, nie kopii samego pliku przy aktywnym WAL. Potwierdzenie „zapisano” następuje po zatwierdzeniu. Wygaszenie aplikacji, rollback oraz brak miejsca mają testy. [Transakcje rusqlite](https://docs.rs/rusqlite/latest/rusqlite/struct.Transaction.html).

Sekrety domyślnie są sesyjne, jak obecnie. Zapis w menedżerze poświadczeń OS może dojść jako osobna, testowana funkcja; nie zastępujemy go plaintextem. Pełna szyfrowana baza, cloud i automatyczne aktualizacje nie są warunkiem tej migracji. Podpisywanie instalatorów i weryfikacja wydania są częścią R9; ewentualny certyfikat/tożsamość wydawcy to zależność zewnętrzna, której nie zastąpi zmiana kodu ani samo CI.

reqwest wymaga świadomej konfiguracji klienta: timeout całego response i poszczególnych etapów, limity streamu, polityka proxy, TLS, redirect i retry. Dokumentacja wskazuje domyślne ponowienia niektórych NACK i brak domyślnego całkowitego timeoutu; nie polegamy na tych ustawieniach przy testach tworzących zasoby. [ClientBuilder](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html). Anulowanie sygnalizuje token, ale polityka kończenia requestu i uruchomienia cleanup należy do naszego runnera. [CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html).

Testy widgetów mogą korzystać z [egui_kittest](https://docs.rs/egui_kittest/latest/egui_kittest/), który daje narzędzia interakcji i kontroli dostępnych nazw. Nie zastępują testów integracji z rzeczywistym oknem, schowkiem i dialogami systemowymi. Snapshoty grafiki są uzupełnieniem sprawdzeń semantycznych, nie jedynym kryterium.

## Najbliższe konkretne zlecenie implementacyjne

Pierwszy przyrost R0/R1 jest opisany poniżej. Następna praca domyka model domenowy i kontrakt wykonania (nowe typy ID, reguły operatorów i aktywacji), sprawdza natywny frontend na Windows oraz rozszerza testy interakcji adaptera grafu. Potem odblokowuje równoległe prace nad runnerem, SQLite i komendami edycji. Obecne źródła TypeScript pozostają referencją; termin ich usunięcia wyznacza R10.

## Dziennik pierwszego przyrostu

- Workspace Cargo, toolchain 1.99.0, Cargo.lock, core bez zależności GUI i natywny frontend eframe.
- Wybrano zgodny zestaw **eframe/egui 0.36.2 + egui-snarl 0.12.0**: manifest snarl wymaga egui 0.36. Przejście na 0.37 wymaga zgodnego adaptera lub aktualizacji snarl; nie łączymy dwóch wersji egui w publicznych interfejsach. To decyzja zgodności, a nie deklaracja najnowszej wersji.
- Import projektu i workspace v1 jest ograniczony do 16 MiB. DTO zachowuje kolejność map, OptionalJson odróżnia brak i null, nieznane pola i duplikaty kluczy są odrzucane zamiast gubione. Wersja obecnego formatu zachowuje limity 200 klocków / 1000 krawędzi na graf.
- Walidator sprawdza powiązania (także zagnieżdżone), parametry, porty, cykle danych/sterowania, IF, Start i rewizje/rekurencję komponentów. Nie jest jeszcze interpreterem ani pełnym kompilatorem wykonania.
- Podgląd egui pokazuje klocki, przewody danych/sterowania i osobne fazy. Pozycje zmienione w podglądzie są stanem widoku; nie są zapisywane do projektu. Żaden request nie jest wykonywany podczas importu. Import jest transakcyjny z punktu widzenia UI: nieudany plik pozostawia bieżący dokument.
- Korpus: cztery projekty i workspace, manifest SHA256 oraz generator TypeScript z `--check`. Node służy tylko do regeneracji porównawczej, a nie do startu binarki Rust.
- Google Antigravity przez `agy`, model `gemini-4-argon-medium`, otrzymał dwa ograniczone zadania: CI i generator fixture. CI powstało poprawnie w zakresie zlecenia. Pierwsze podejście generatora zatrzymało się na ograniczeniu uprawnień headless; drugie wykonało zapis bez komend. Koordynator poprawił błędny import typu oraz deterministyczne sortowanie manifestu i sam uruchomił weryfikację. Dalszy kod migracji napisał koordynator.
- CI definiuje build/test/clippy i artefakty dla Linux oraz Windows; samo dodanie YAML nie oznacza, że te joby wykonały się na zdalnych runnerach. Podpisane pakiety, kompletne GUI Windows i cała macierz wydania pozostają wymaganiami R9/R10.

Instrukcje uruchomienia aktualnego kodu: [docs/migration/development.md](docs/migration/development.md).
