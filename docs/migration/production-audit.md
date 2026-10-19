# Audyt wymagań wydania Rust / egui

Status: plan wymagań i odbioru, nie deklaracja gotowości produkcyjnej. Audyt dotyczy kodu dostępnego w repozytorium 19.10.2026. Docelowo aplikacja desktopowa dla jednego użytkownika. Użytkownik potwierdził Linux i Windows w pierwszym wydaniu; obie platformy wymagają natywnego CI, artefaktów i testów instalacji. Proponowany zakres Windows: 10/11 x86_64; dokładne wspierane wydania Windows oraz dystrybucja i minimalna glibc Linux wymagają zatwierdzenia przed budowaniem pakietów. macOS pozostaje poza pierwszym wydaniem.

## Co istnieje i co trzeba zachować

| Obszar         | Dowód w aktualnym kodzie                                                                                                                         | Wniosek dla migracji                                                                                                                                                                       |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Zapis projektu | `app/features/workspace/storage.ts`, `Workspace.tsx`: JSON v1 w localStorage, walidacja, eksport, debounce 350 ms, flush przy opuszczeniu strony | Zaimportować v1 bez utraty konfiguracji; przeglądarka i desktop nie współdzielą storage. Wymagany jawny eksport z dotychczasowej aplikacji i import w desktopie.                           |
| Migracje       | Obecna migracja zmienia dawny adres lokalnego demo; obcy numer wersji jest odrzucany                                                             | Potrzebne jawne, testowane migracje formatu, kopia źródła i bezpieczne zachowanie przy nowszej wersji.                                                                                     |
| Sekrety        | `publicProject` usuwa wartości zadeklarowanych sekretów; wartości sesyjne są w pamięci                                                           | Nie przenosić tokenów do ustawień egui, projektu, schowka bloczków ani raportów.                                                                                                           |
| Raport         | Runner maskuje nazwy wrażliwe i znane wartości, frontend dodatkowo maskuje eksport                                                               | Zachować centralną redakcję przed publikacją zdarzeń. Heurystyka nazw i wyszukiwanie wartości nie gwarantują usunięcia każdej transformacji sekretu ani danych osobowych.                  |
| HTTP           | `api/runner/index.ts`: HTTP/HTTPS, brak danych logowania w URL, brak przekierowań i retry; 4xx/5xx pozostają danymi                              | Zachować semantykę oraz jawnie udostępnić obsługiwaną konfigurację transportu.                                                                                                             |
| Limity         | Timeout requestu domyślnie 10 s, maks. 60 s; odpowiedź 2 MiB; budżet main 120 s i cleanup 10 s; budżety grafu/raportu                            | Przenieść limity do typowanej polityki wykonania i egzekwować także podczas streamowania oraz dekodowania. Nie traktować limitu samego Content-Length jako wystarczającego.                |
| Kolejność i IF | `api/runner/control.ts`, testy workflow: Start, serialny DAG, wybór gałęzi, zależności danych                                                    | Kontrakt wykonania jest cechą produktu. Przepisanie na async Rust nie może niejawnie wprowadzić równoległych requestów.                                                                    |
| Anulowanie     | Main honoruje sygnał; cleanup ma niezależny budżet. Awaria procesu nie gwarantuje sprzątania                                                     | Zachować osobne stany anulowania i sprzątania; nie obiecywać cofnięcia skutków requestu.                                                                                                   |
| Historia       | `api/routes/runs.ts`: pamięć sesji, 4 aktywne runs, do 100 zakończonych, próg 32 MiB                                                             | Ustalić jawny limit/retencję desktopu. Obecny próg pamięci nie ogranicza twardo rozmiaru wszystkich aktywnych runs.                                                                        |
| Testy          | Testy modelu, runnera z lokalnym serwerem, workflow, sekretów, komponentów, Playwright dla UI                                                    | Zachować zestaw fixtures jako oracle; nie deklarować pokrycia desktopu na podstawie testów przeglądarki.                                                                                   |
| Wydawanie      | Skrypty typecheck/test/build; istniejący Dockerfile dotyczy aplikacji Node                                                                       | Brak wykazanego pipeline wydania desktopu. Dockerfile finalnie nie kopiuje `api`/`shared`, a `npm start` wymaga `tsx` z devDependencies; nie stanowi działającej ścieżki dystrybucji Rust. |

## Granica zaufania i jawny zakres bezpieczeństwa

Projekt importowany z pliku jest niezaufany. Importowanie, podgląd i migracja nie mogą wysyłać requestów, rozwiązywać sekretów ani odtwarzać sprzątania. Walidacja obejmuje rozmiar pliku, liczbę elementów, głębokość JSON i wyrażeń, duplikaty ID, zakres liczb, porty, referencje, rewizje oraz cykle. Edytor nie wykonuje dowolnego JS/Rust ani komend systemowych z projektu.

Runner API testing świadomie ma dostęp do localhost i sieci prywatnych; ich ogólna blokada uniemożliwiłaby podstawowe zastosowanie. Wydanie lokalne ufa użytkownikowi uruchamiającemu scenariusz. Interfejs pokazuje tryb Live, środowisko i docelowy host; uruchomienie wymaga jawnej akcji. Projekt może zawierać pełny URL pomijający base URL, co musi być widoczne przy przeglądzie konfiguracji. Publiczny lub zdalny wieloużytkownikowy runner wymaga osobnego modelu autoryzacji i izolacji sieci i nie należy do tego wydania.

Natywna aplikacja powinna wykonywać runner bez otwartego portu HTTP: komunikaty i kanały wewnątrz procesu, z oddzieleniem UI od runtime. Jeśli lokalny daemon okaże się wymagany, jego uwierzytelnienie, IPC, uprawnienia i lifecycle stają się osobnym blokującym zadaniem; nie należy przenosić obecnego publicznego kontraktu localhost bez analizy.

## Bramki wydania

Każda bramka wymaga dowodu z CI lub zapisanego protokołu testu na docelowej platformie. Niespełnienie bramki zatrzymuje stabilne wydanie; ukończenie samego kodu nie zamyka zadania.

### G1 — zgodność danych i odzyskiwalny zapis

1. Zdefiniować osobno wersję dokumentu projektu i wersję ustawień aplikacji. Wersję czytać przed deserializacją właściwego modelu. Nowszą wersję otwierać z czytelnym błędem bez modyfikacji oryginału.
2. Projekty zapisywać jako przenośne, wersjonowane dokumenty JSON. Stan UI i indeks ostatnich plików przechowywać oddzielnie; nie mieszać sekretów ze stanem `eframe`.
3. Zapis pliku: walidacja modelu → plik tymczasowy w tym samym katalogu → flush/sync → atomowe zastąpienie → synchronizacja katalogu tam, gdzie platforma pozwala. Zachować ostatnią poprawną kopię i procedurę odtworzenia. Obsłużyć brak miejsca, uprawnień i przerwanie operacji; po błędzie UI nadal pokazuje niezapisane zmiany.
4. Autosave/recovery przechowywać pod identyfikatorem dokumentu, z uprawnieniami użytkownika. Po awarii zaoferować odzyskanie, pokazać datę i źródło, nie nadpisywać pliku bez wiedzy użytkownika.
5. Dwie instancje edytujące plik muszą wykryć konflikt przez blokadę lub rewizję/hash; brak cichej utraty ostatniego zapisu. Zewnętrzna zmiana pliku również wymaga wykrycia.
6. Migracja v1 importuje wszystkie rodzaje klocków, zagnieżdżone bindingi, template inputs, rewizje, kontrakty, Start/IF, cleanup oraz pozycje. Sekrety deklarowane w starym pliku usuwa się z modelu przeznaczonego do zapisu i nie drukuje w błędzie walidacji. Zachować źródłowy plik; migracje nie kontaktują się z demo API.

Dowody: golden fixtures z aktualnych przykładów i projektów granicznych; import → save → reopen zachowuje semantykę; testy uszkodzonego/truncated JSON, future-version, przerwania każdego etapu zapisu, konfliktu i disk-full (przez fault injection). Recovery sprawdzone na rzeczywistym filesystemie Linux i Windows, z uwzględnieniem różnic atomowego zastępowania pliku i blokad.

### G2 — sekrety, prywatność i raportowanie

1. Domyślnie sekrety pozostają sesyjne. Trwały magazyn poświadczeń jest opcjonalnym późniejszym zakresem; jeśli wejdzie do wydania, wymaga systemowego keyringu, jawnej zgody na zapis, obsługi zablokowanego/brakującego keyringu i zakazu cichego fallbacku do plaintext.
2. Typy `SecretRef`/`SecretValue` oraz jeden resolver oddzielają model projektu od poświadczeń. Brak sekretu daje diagnostykę wskazującą nazwę, bez wartości. `Debug`, `Display`, błędy i tracing nie mogą ujawniać wartości.
3. Zdarzenia UI, eksport i trwała historia przechodzą przez wspólny mechanizm redakcji. Raw request/response są potrzebne do bieżących obliczeń runnera, ale nie mogą znaleźć się przypadkiem w logu, `Debug` taska ani panic payloadzie. Wrażliwe nagłówki są maskowane niezależnie od deklaracji sekretów.
4. Obsłużyć znane wartości i ich formy URL-encoded; testować echo sekretu w dowolnym polu, błędzie, URL, query, nagłówkach, body i wnętrzu komponentu. Wspierać dodatkowe ścieżki pól do maskowania odpowiedzi oraz tryb raportu z metadanymi bez payloadów.
5. Historia sesyjna i domyślnie brak telemetrii/persistowania payloadów. Ewentualny zapis historii wymaga jawnego ustawienia retencji, limitu miejsca, kasowania i ostrzeżenia o wrażliwych danych odpowiedzi. Log techniczny domyślnie zawiera identyfikatory i kody błędów, nie body ani pełne URL z query.
6. Opisać ograniczenia: arbitralnie zakodowany/zaszyfrowany/połączony sekret i PII pod dowolnym polem nie są automatycznie wykrywalne. Zrzuty pamięci i logi testowanego API są poza gwarancją redakcji aplikacji. Nie obiecywać kryptograficznego wymazania wszystkich kopii w pamięci.

Dowody: canary secrets w macierzy miejsc i transformacji; inspekcja eksportu, plików autosave, logów i zdarzeń; brak wartości w schowku bloczków. Osobny test blokuje pomylenie zredagowanej historii z surowymi danymi do wykonania pojedynczego kroku.

### G3 — HTTP przewidywalne w środowisku użytkownika

1. Zachować: statusy HTTP są danymi, brak automatycznych retry i przekierowań, brak odziedziczonego cookie jar między niezależnymi przebiegami. Ponowienie POST po timeout nie może następować automatycznie.
2. TLS weryfikuje certyfikat i nazwę hosta. Nie dodawać globalnego `accept invalid certificates`. Wydanie wymaga wskazanej polityki trust store: systemowe CA oraz jawny dodatkowy CA per środowisko, bez mutacji globalnego magazynu systemu. Testy lokalnego CA obejmują wygasły certyfikat i zły hostname.
3. Proxy musi być jawne i widoczne: bezpośrednie albo konfigurowane per środowisko, w tym `no_proxy`; nie odziedziczyć przypadkiem proxy procesu i nie wyciekać poświadczeniami w logach. Jeśli początkowo proxy nie jest obsługiwane, wydać to jako udokumentowaną nieobsługiwaną konfigurację, bez pozornego pola w UI.
4. Osobne limity connect i całego requestu; cancellation musi przerywać także czytanie wolnego body. Limit wielkości obejmuje rozkodowaną odpowiedź i dane przekazane parserowi, a ponadto request body, liczbę/wielkość nagłówków, głębokość JSON i budżet raportu. Błąd limitu zachowuje metadane bez publikowania częściowego wrażliwego body.
5. Widoczne rozróżnienie DNS, connect, TLS, timeout, cancellation, przekroczenia limitu, niepoprawnego JSON i błędu konfiguracji. Treść błędu transportu przechodzi redakcję.
6. Ustalić i pokazać reguły charset, kompresji, nie-JSON response, query duplicate keys i pustego body. Migracja zachowuje obecne znaczenie base URL jako prefiksu oraz typed body interpolation.

Klienty mTLS, OAuth flows, upload multipart, WebSocket i obsługa redirect/retry mogą pozostać poza pierwszym wydaniem. Dodatkowy CA jest wymagany dla zadeklarowanego profilu pracy z wewnętrznymi API; jeśli profil zostanie zawężony do publicznych CA, należy zmienić kryterium jawnie przed implementacją, a nie pominąć je przy odbiorze.

Dowody: wyłącznie kontrolowane serwery fixture dla HTTP/TLS/proxy, chunked/slow/chunked-compressed response, błędnego JSON, certyfikatów i przerwania po przyjęciu POST. Smoke test połączenia na maszynie odbiorczej; brak zależności testów od publicznego API.

### G4 — wykonanie, anulowanie i sprzątanie

1. UI nie blokuje się na sieci, parsowaniu i filesystemie. Runtime komunikuje się przez ograniczone kolejki; duży/aktywny raport nie może doprowadzić do nieograniczonego buforowania ani zatrzymać przycisku Anuluj.
2. Run działa na niezmiennym snapshotcie grafu, środowiska i rewizji komponentów. Każde zdarzenie ma run ID i stabilną tożsamość kroku wraz ze ścieżką instancji komponentu. Zmiana edytora nie zmienia aktywnego run.
3. Wewnątrz scenariusza wykonanie jest serialne i deterministyczne. Niewybrana gałąź nie wysyła requestów; brak danych z gałęzi pominiętej nie uruchamia jej ponownie. Scenariusze starsze bez Startu zachowują jawny tryb zgodności.
4. Dopuszczalna liczba jednoczesnych run ma twardy limit (początkowo zachować maks. 4), a kolejki mają granice. Buforowanie danych i raportów musi mieć łączny budżet, nie tylko per-response.
5. Maszyna stanów definiuje dokładnie jeden stan końcowy oraz status każdego kroku. Anulowanie blokuje rozpoczynanie kolejnych kroków main i uruchamia cleanup z niezależnym budżetem. Powtórne Cancel jest idempotentne; UI pokazuje, że cleanup nadal trwa.
6. Cleanup uruchamia się również po błędzie komponentu i timeout; operuje wyłącznie na dostępnych bieżących danych. Awaria cleanup nie maskuje pierwotnego błędu i jest widoczna osobno. Zamknięcie okna podczas run oferuje dokończenie sprzątania lub jawne zakończenie procesu.
7. Żaden mechanizm nie gwarantuje, że request anulowany po wysłaniu nie został wykonany przez serwer. Po twardej awarii journal oznacza run jako przerwany z nieznanym wynikiem skutków; nie wznawia automatycznie requestów ani cleanup. Użytkownik dostaje kontekst do ręcznej weryfikacji.

Dowody: konformacyjne fixtures TS/Rust dla workflow i wszystkich operatorów; anulowanie na każdym etapie (także TLS, body i cleanup); zamknięcie aplikacji, duże raporty, konflikt edycji i run, wielokrotne Cancel. Test cart: ceny → IF 200 → utwórz pusty koszyk → dodaj produkty → porównaj sumę → usuń koszyk, a gałąź 404 nie tworzy koszyka.

### G5 — testy i pipeline wydania

1. Rust workspace ma przypięty toolchain, MSRV, lockfile, `fmt --check`, `clippy` z wybranym limitem ostrzeżeń, unit/integration/doc tests, analizę zależności i licencji oraz build release w CI. Egzekwować aktualizację lockfile przez kontrolowany proces, nie przypadkowe pobieranie najnowszych wersji w release.
2. Domain/runner testuje się bez egui. Fixture parity pokrywa null/missing/false/zero, liczby i Unicode, operatory losowe kontrolowane seedem, template aliases, snapshoty, rewizje, IF merge, cleanup oraz import starszego formatu. Różnice semantyki Rust/JS muszą być opisane i zatwierdzone testem.
3. Property/fuzz tests obejmują parser/import, referencje i cykle, JSON path, remap ID przy kopiowaniu, migracje i politykę redakcji. Fuzz nie zastępuje ręcznej analizy bezpieczeństwa.
4. Automatyczne scenariusze UI desktopu obejmują utworzenie grafu, drag/drop zmiennej, wybór pola odpowiedzi, IF, kopiowanie grupy z konfiguracją, undo/redo, resize raportu, zapis/reopen i anulowanie. Zewnętrzne testy przeglądarki pozostają testami starego UI.
5. Artefakt wydania jest budowany przez CI, posiada checksumy, manifest wersji/toolchainu i listę licencji; uruchamia się na czystej maszynie bez Node, npm ani repozytorium. Linux i Windows mają oddzielne natywne joby CI oraz test instalacji, uruchomienia, aktualizacji i deinstalacji na czystej maszynie/VM. Dla Linux ustalić minimalną wersję glibc, dystrybucję bazową, architekturę i wymagane biblioteki graficzne; zweryfikować Wayland i X11. Proponowany baseline do potwierdzenia: Ubuntu 22.04 x86_64 / glibc 2.35. Dla Windows ustalić wspierane wydania 10/11 x86_64, podpisany instalator, zakres uprawnień per-user i działanie bez zainstalowanego toolchainu; sprawdzić ścieżki z Unicode i spacje.
6. Artefakty/manifest mają weryfikowalny podpis wydawcy oraz dokumentowaną procedurę przechowywania i rotacji klucza CI. Windows wymaga podpisu instalatora i binarek oraz weryfikacji podpisu w smoke teście; oddzielne natywne testy są obowiązkowe. macOS nie jest objęty tym wydaniem; dodanie go później wymaga codesign/notarization i własnej macierzy testów. Cross-kompilacja nie zastępuje testów uruchomienia.
7. Pierwsze stabilne wydanie może aktualizować się ręcznie z podpisanego artefaktu. Auto-update nie jest wymagany; jeśli zostanie włączony, wymaga podpisanego manifestu, weryfikacji artefaktu, ochrony przed downgrade i zachowania danych przy nieudanej aktualizacji.
8. Release checklist obejmuje odtworzenie projektu z backupu, aktualizację ze wspieranej wersji, cofnięcie binarki bez uszkodzenia danych, znane ograniczenia i instrukcję zebrania diagnostyki bez sekretów.

### G6 — używalność, dostępność i mierzalna wydajność

Do czasu uzgodnienia sprzętu referencyjnego poniższe liczby są proponowanymi celami odbioru, a nie wynikami pomiarów: Linux i Windows x86_64, 4 rdzenie, 8 GiB RAM, zintegrowane GPU, 1920×1080, skala 100% i 200%. Pomiar trzeba wykonać osobno na obu platformach.

- Graf 200 węzłów / 1000 połączeń: p95 czasu klatki podczas pan/zoom/drag ≤ 16,7 ms; p99 ≤ 33,3 ms. Dodatkowy syntetyczny profil 500 węzłów / 1000 przewodów: p95 ≤ 33 ms; ten profil obciążenia nie rozszerza automatycznie limitu importu v1. Zmierzyć z normalnymi etykietami i otwartym inspectorem, nie pustymi prostokątami.
- Zarejestrowanie Cancel w UI ≤ 100 ms; zatrzymanie lokalnego fixture request i przejście do cleanup ≤ 1 s. Nie obejmuje cofnięcia operacji po stronie testowanego serwera.
- Start pustego projektu do interaktywnego okna ≤ 2 s; pamięć ustabilizowana dla grafu testowego ≤ 500 MiB poza jawnie zadeklarowanym budżetem aktywnych payloadów. Serie 100 run nie wykazują monotonicznego przyrostu pamięci po zakończeniu i usunięciu raportów.
- Raport z dużym JSON renderuje się przyrostowo/wirtualnie; brak rozwijania całego drzewa i serializacji pełnego raportu w każdej klatce.
- Wszystkie kluczowe operacje dostępne klawiaturą: fokus i wybór węzłów, tworzenie połączenia przez formularz/listę, edycja argumentu, wybór pola, Start/Cancel, copy/paste/undo i resize raportu. Przeciąganie nie jest jedyną metodą obsługi.
- Widoczny fokus, czytelność błędów i stanów bez polegania wyłącznie na kolorze, skalowanie fontów, kontrast i poprawne polskie znaki. Dialogi mają przewidywalny fokus i Escape; skróty grafu nie przechwytują edycji tekstu.
- Dostępność egui przez dostępny backend musi zostać sprawdzona z czytnikiem ekranu na wspieranej platformie. Własny canvas wymaga semantycznej alternatywy listy kroków i formularzy połączeń. Sama obecność biblioteki dostępności nie zamyka odbioru.

Dowody: profil obciążenia i surowe wyniki pomiarów z identyfikatorem builda/sprzętu; ręczny protokół klawiatury/czytnika; testy skalowania i screenshoty. W razie nieosiągnięcia celu najpierw profilowanie, potem zmiana implementacji albo jawna korekta wspieranego limitu.

## Kolejność zadań i odpowiedzialność

Poniższe P0–P7 to lokalny podział audytu. Nadrzędny backlog, właściciele i etapy R0–R10 znajdują się w [RUST_MIGRATION.md](../../RUST_MIGRATION.md). Magazyn roboczy to SQLite; wymagania atomowego zapisu plików dotyczą eksportu i plików pomocniczych, nie zastępują transakcji i spójnego backupu SQLite.

| Zadanie                                       | Zależności                                          | Właściciel modułu               | Warunek zamknięcia                                                                                 |
| --------------------------------------------- | --------------------------------------------------- | ------------------------------- | -------------------------------------------------------------------------------------------------- |
| P0: kontrakt kompatybilności i fixture corpus | Audyt obecnego modelu i runnera                     | Domain / koordynator            | Zapisane reguły i fixtures obejmujące funkcje istniejącego produktu; brak cichej zmiany znaczenia. |
| P1: import/migracje i storage recovery        | P0, decyzja o platformie                            | Persistence                     | G1 i testy fault injection.                                                                        |
| P2: typy sekretów, redakcja, log policy       | P0                                                  | Security / runner               | G2, zanim pierwsze zdarzenie runnera trafi do UI lub pliku.                                        |
| P3: klient HTTP, budżety, runtime             | P0 + P2, decyzje TLS/proxy                          | Runner                          | G3 oraz backpressure i limit pamięci.                                                              |
| P4: scheduler, anulowanie, cleanup            | P0 + P3                                             | Runner                          | G4 i cart fixture, zgodność ze starym silnikiem.                                                   |
| P5: aplikacja desktopowa i interakcje         | P0; integracja P1/P4                                | UI                              | Zachowana pełna lista funkcji; G6 i automatyczne scenariusze UI.                                   |
| P6: CI/artefakty                              | Kontrakt workspace i platformy; równolegle do P1–P5 | Release / koordynator           | Reprodukowalna procedura builda, podpis, czysty smoke test.                                        |
| P7: kandydat stabilnego wydania               | P1–P6                                               | Koordynator + niezależny review | Wszystkie G1–G6 z dowodami; otwarte defekty sklasyfikowane i brak blokujących.                     |

Dalsze funkcje, które nie blokują zgodnego pierwszego wydania: współpraca zespołowa, chmura, publiczny runner, automatyczne OAuth, keyring, wydania macOS i inne poza zadeklarowaną macierzą Linux/Windows, automatyczne aktualizacje, pętle i równoległe wykonanie grafu. Nie należy używać tego odroczenia do pominięcia poprawności danych, bezpieczeństwa lokalnego wykonania ani istniejących funkcji edytora.
