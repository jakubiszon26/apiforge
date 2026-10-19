# API Forge

Rozpoczęta migracja: [Rust + egui — migracja produkcyjna](RUST_MIGRATION.md). Docelowo Linux i Windows od pierwszego wydania; obecny kod poniżej pozostaje wersją referencyjną na czas migracji.

Lokalny prototyp wizualnych testów regresji REST API. Zespół definiuje własne requesty, komponenty, kontrakty danych i akcje biznesowe; silnik nie zakłada żadnej domeny ani nazw endpointów.

Natywny podgląd Rust i komendy sprawdzania opisuje [instrukcja developerska](docs/migration/development.md). Obsługuje import projektu/workspace v1; pełny edytor i runner są w trakcie migracji.

## Uruchomienie

```bash
npm install
npm run dev
```

Otwórz **http://127.0.0.1:5173**. Frontend działa na porcie 5173, a lokalny runner na 4318. Projekt zawiera lokalną zależność Node 22 zgodną z wymaganiami React Router; polecenia `npm run` korzystają z niej automatycznie. Serwery nasłuchują wyłącznie lokalnie.

Wariant po zbudowaniu:

```bash
npm run build
npm start
```

Backend udostępnia wtedy zbudowany interfejs pod **http://127.0.0.1:4318**. Przy pracy w trybie developerskim używaj portu 5173.

## Pierwszy scenariusz

1. Otwórz przykładowy projekt sklepu albo systemu zgłoszeń. Możesz też utworzyć pusty projekt.
2. Nowy scenariusz zawiera ręczny **Start**. Dodaj klocek z biblioteki kliknięciem lub przeciągnięciem. Skonfiguruj go w panelu po prawej; wybrane argumenty są edytowalne bezpośrednio na klocku.
3. Połącz **Dalej** z górnym wejściem kolejnego klocka albo wybierz **Następny krok** w konfiguracji. Pierwszy request nowego scenariusza łączy się ze Startem automatycznie. Kliknij klocek prawym przyciskiem albo wybierz **Akcja**. Dostępne akcje projektu wynikają z typu wyjścia. Wybrana akcja tworzy połączony krok.
4. Podepnij źródła danych do nazwanych wejść. Możesz też wybrać wcześniejszy krok i ścieżkę JSON w konfiguracji argumentu.
5. W zakładce danych wyjściowych kliknij pole odpowiedzi, aby dodać jego sprawdzenie.
6. Wybierz **Symulacja** lub **Rzeczywiste API** i uruchom test. Symulacja używa jawnie zapisanych przykładów; rzeczywisty przebieg wysyła requesty przez backend.
7. Otwórz wynik kroku: raport pokazuje wejścia, wyjścia, request i response, wartości oczekiwane i otrzymane oraz wnętrze własnych komponentów.

Przy pełnym uruchomieniu projektu sklepu tryb rzeczywisty korzysta z lokalnego API demonstracyjnego, tworzy koszyk, sprawdza cenę i usuwa koszyk w sekcji **Po teście**. Produkt `123` ma cenę promocyjną 80, a `124` — 90. Zmiana ID przy oczekiwanej cenie 80 pozwala zobaczyć wykrytą rozbieżność.

Dane demonstracyjne są zdefiniowane w `shared/demos.ts` i `api/demo.ts`; nie należą do silnika wykonania.

## Kolejność i IF / ELSE

W bibliotece wybierz **Przykład: kolejność i IF**, a następnie **API → Uruchom test**. Przepływ pobiera cenę produktu, sprawdza status 200 w IF, tworzy pusty koszyk i dopiero potem dodaje produkty. ID koszyka i dane produktu pochodzą z aktualnych odpowiedzi; końcowe Porównanie sprawdza sumę. Sekcja **Po teście** usuwa koszyk.

**Start** uruchamia połączoną ścieżkę. Każdy request kończy się przed rozpoczęciem następnego. **IF / ELSE** przyjmuje takie same argumenty i operacje jak Porównanie, ale wynik wybiera wyjście **Tak** lub **Nie**. Fałsz sam w sobie nie jest błędem testu. Połącz wyjścia z istniejącymi klockami przez przeciągnięcie lub listy w konfiguracji; kliknięcie **Dalej / Tak / Nie** na klocku pozwala dodać nowy krok od razu z połączeniem.

Kroki poza ścieżką od Startu i niewybrane gałęzie są pomijane bez requestów. Dane dodatkowo ustanawiają zależności: blok czeka na źródła wszystkich argumentów. Alternatywne połączenia kolejności mogą schodzić się do wspólnego kroku; wystarcza aktywna gałąź, ale argumenty nadal muszą być dostępne. Odwołanie do odpowiedzi pominiętej gałęzi nie uruchomi jej ponownie. Starsze zapisane grafy bez Startu zachowują poprzednie wykonanie wszystkich korzeni i mają widoczną informację o tym trybie. Dodaj Start i połącz ścieżkę, aby ograniczyć uruchamiane kroki.

W przykładzie zmień endpoint produktu na `/products/999`, aby zobaczyć IF → Nie i brak tworzenia koszyka. Gałąź Nie zawiera Porównanie zgłaszające nieoczekiwany status.

## Własne komponenty

Zaznacz kroki na grafie (Ctrl/Cmd + klik) i zapisz zaznaczenie jako komponent. Wybierz publiczne argumenty, zwracane dane i opcjonalny argument zasilany z menu **Dodaj akcję**. Komponent może zawierać inne komponenty. Kontrakty danych definiujesz w ustawieniach projektu.

Edycja wnętrza komponentu tworzy nową rewizję. Istniejące instancje pozostają przy swojej rewizji; wybraną instancję aktualizujesz w jej konfiguracji. Oryginalny fragment scenariusza pozostaje na grafie po zapisaniu komponentu do biblioteki.

## Dane i wykonanie

- Projekty zapisują się lokalnie w przeglądarce. Eksport/import JSON pozwala zachować kopię i przenieść definicje.
- Sekrety środowiska są przechowywane tylko w pamięci bieżącej sesji i nie trafiają do eksportu projektu. Nagłówki autoryzacji oraz rozpoznane wartości tajne są maskowane w raportach. Nie wpisuj tokenów jako zwykłych wartości literalnych w definicji projektu.
- Base URL może zawierać prefiks, np. `https://example.test/api/v1`. Ścieżka `/users/:id` jest do niego dołączana; pełny URL pomija base URL.
- Dostępne są argumenty ścieżki `:argument`, podstawienia `{{argument}}` w URL/query/nagłówkach oraz body JSON. Całe podstawienie w wartości body zachowuje typ argumentu.
- Status HTTP 4xx/5xx jest danymi do sprawdzenia. Dodaj asercję statusu, jeśli chcesz zaliczać tylko wybrane odpowiedzi.
- Po błędzie można zatrzymać scenariusz lub kontynuować możliwe kroki. Niespełniony warunek danych blokuje zależne kroki. Sprzątanie ma osobny przebieg i wykonuje się także po błędzie lub kontrolowanym anulowaniu.
- Wykonanie do wybranego klocka jest częściowym przebiegiem. Pojedynczy krok może korzystać z jawnie wskazanych danych wcześniejszego wykonania; jego wynik jest diagnostyczny.

## Weryfikacja

```bash
npm run typecheck
npm test
npm run test:e2e
npm run build
```

Testy silnika korzystają z kontrolowanego lokalnego serwera. Testy przeglądarkowe używają systemowego `/usr/bin/chromium`, jeśli jest dostępny; w innym przypadku wymagają Chromium dla Playwright (`npx playwright install chromium`). Można wskazać przeglądarkę przez `PLAYWRIGHT_CHROMIUM_EXECUTABLE`. Konfiguracja testów uruchamia frontend/backend lub korzysta z już uruchomionego podglądu.

## Ograniczenia prototypu

- Jeden użytkownik, lokalna aplikacja; brak współpracy w czasie rzeczywistym i publicznego hostingu runnera.
- Acykliczne grafy wykonywane sekwencyjnie; bez pętli, macierzy przypadków, zaawansowanych rozgałęzień i skryptów JavaScript.
- Requesty HTTP z JSON-em lub odpowiedzią tekstową; brak multipart, automatycznej obsługi OAuth, automatycznych retry i przekierowań HTTP.
- Timeout requestu do 60 sekund, limit odpowiedzi 2 MiB, budżet scenariusza 120 sekund i osobne 10 sekund na sprzątanie.
- Historia wyników jest sesyjna i ograniczona w pamięci backendu; projekt można zapisać trwale, raport wyeksportować.
- Anulowanie nie cofa operacji wykonanych przez testowane API. Awaria procesu nie gwarantuje wykonania sprzątania.

Specyfikacja zakresu: [PROTOTYPE.md](PROTOTYPE.md). Podział prac i kryteria: [AGENT_TASKS.md](AGENT_TASKS.md). Wcześniejsza makieta znajduje się w `design-preview/`.

Nowy request zaczyna bez argumentów. Nazwij własne argumenty w konfiguracji, określ ich typ i wybierz, które pokazać na klocku. Nazwy mogą odpowiadać filtrom, tekstom, flagom, obiektom czy parametrom ścieżki; `id` nie ma specjalnego znaczenia.

Przy polu w danych wyjściowych lub raporcie wybierz **Sprawdź**. Powstanie komponent **Porównanie** z argumentami: wartością otrzymaną i oczekiwaną. W jego konfiguracji wybierzesz operację (np. równość, zawieranie lub „Ma typ”). Dla „Ma typ” wybierzesz oczekiwany typ z listy. Oba argumenty mogą pochodzić z wpisanej wartości, wyniku innego kroku, parametru lub środowiska.

Formatowanie kodu: `npm run format`; kontrola bez zmian: `npm run format:check`.

PoC bloków operatorów: w bibliotece kliknij **Otwórz przykład blokowy**. Projekt zawiera odczyt lokalnego API oraz dwa porównania z zagnieżdżonymi operatorami. Wybierz klocek porównania, aby zobaczyć zielone bloki w jego argumentach. Przycisk **API** uruchamia ten sam scenariusz na lokalnym serwerze demonstracyjnym.

W dowolnym argumencie wybierz źródło **Operator · zagnieżdżony blok** lub przeciągnij operator z zielonej palety na argument. Kolejne gniazda mogą zawierać inne operatory, wartości, parametry, zmienne środowiska i pola wcześniejszych odpowiedzi. Kliknięcie operatora w palecie dodaje go jako osobny krok grafu. Operator zwraca wartość; **Porównanie** sprawdza jej zgodność z oczekiwaniem. „Typ wartości” jest operatorem zwracającym tekst, np. `number`, `array` lub `null`.

Dostępnych jest 37 operatorów w sześciu grupach. Matematyka i logika wymagają właściwych typów: brak danych i null nie są zerem. Reguły, zakres PoC i kolejne etapy opisuje [BLOCKS_POC.md](BLOCKS_POC.md). Pętle i rozgałęzienia wykonania są poza obecnym PoC.

W argumencie ze źródłem „Wynik innego kroku” przycisk **Wybierz pole…** otwiera wyszukiwarkę pól z typami i podglądem wartości. Korzysta z ostatniej odpowiedzi wybranego kroku (także po zmianie porównania) lub przykładowych danych; pochodzenie jest widoczne. Można wybrać całe body, zagnieżdżone pole, element listy albo wpisać własną ścieżkę.

Sekcja **Zmienne** w bibliotece zawiera przeciągane bloczki. Przycisk **+** dodaje do grafu osobną zmienną z nazwą i wartością dowolnego typu JSON. W tej samej sekcji są parametry scenariusza, argumenty edytowanego komponentu, zmienne środowiska i nazwy sekretów.

Przeciągnij zmienną na wartość argumentu (także wewnątrz operatora lub bezpośrednio na klocku), aby utworzyć powiązanie zachowujące typ. W URL-u, wartości query, nagłówku albo ścieżce „Wybierz dane” upuszczenie wstawia `{{nazwa}}` w miejscu kursora lub zaznaczonego tekstu i automatycznie wiąże źródło. Pomarańczowe bloczki pod tekstem można przeciągać ponownie. Zmiana wartości zmiennej działa w następnym wykonaniu; cykle są odrzucane. Payload przeciągania nie zawiera wartości sekretów.

Aby zmienna pobierała pole odpowiedzi, zaznacz jej bloczek, kliknij **Wybierz wartość z odpowiedzi**, wskaż request i wybierz pole z listy. Możesz też użyć **Do zmiennej** przy polu w danych wyjściowych lub raporcie: utwórz nową zmienną albo wybierz istniejącą. Nazwę pola odpowiedzi można przeciągnąć na wartość zmiennej, również bezpośrednio na grafie.

Przypisanie przechowuje źródło `request → wyjście → ścieżka`, więc przy kolejnym uruchomieniu pobiera świeżą wartość, zachowując typ JSON. Request wykona się przed zmienną, a zmienna przed porównaniem, które jej używa. Brak pola daje błąd zamiast zapamiętanej starej wartości; `null`, `false` i zero pozostają poprawnymi wartościami. Cykl zależności jest odrzucany.

Na planszy zaznacz klocek lub grupę (Ctrl/Cmd + klik). **Ctrl/Cmd+C** kopiuje zaznaczenie z konfiguracją, a **Ctrl/Cmd+V** wkleja niezależną kopię na tej samej planszy. **Ctrl/Cmd+D** duplikuje od razu. Te same akcje są w pasku narzędzi grafu. Schowek planszy przechowuje stan z chwili kopiowania, a kolejne wklejenia dostają nowe ID i przesunięte pozycje. Wewnętrzne połączenia grupy są przepinane do kopii; odwołania do pozostałych klocków pozostają przy swoich źródłach. Wklejenie można cofnąć przez Ctrl/Cmd+Z. W polach tekstowych skróty kopiują i wklejają tekst.
