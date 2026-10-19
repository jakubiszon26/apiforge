# Migracja interfejsu do Rust + egui

Stan audytu: 19.10.2026. Dokument opisuje wymagania migracji, nie deklaruje ich implementacji. Bazą są aktualne pliki aplikacji i testy; sam nowy szkielet okna nie stanowi spełnienia tych wymagań. Zgodnie z decyzją użytkownika Linux i Windows są celami pierwszego wydania; macOS pozostaje poza tym wydaniem. Wszystkie wymagania natywne dotyczą obu wspieranych systemów.

## Inwentaryzacja zgodności funkcjonalnej

| Obszar                 | Zachowanie wymagane w aplikacji natywnej                                                                                                                                                                            | Obecny punkt odniesienia                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Projekty i scenariusze | Wybór, tworzenie, nazwy, ustawienia, import/eksport JSON, trwały zapis, osobne grafy main/cleanup                                                                                                                   | `Workspace.tsx`, `ProjectSettings.tsx`, `storage.ts`, `tests/project.test.ts`                            |
| Plansza                | Przesuwanie i wielokrotne zaznaczanie, pan/zoom, dopasowanie widoku, dodawanie z palety i menu, usuwanie klocków i przewodów, powiązanie zaznaczenia z inspektorem                                                  | `GraphCanvas.tsx`, `graph/operations.ts`                                                                 |
| Przepływ sterowania    | Jeden Start, Dalej, IF z Tak/Nie, odrębne przewody danych; łączenie przez port lub selektor; odrzucenie cykli i niezgodnych portów                                                                                  | `FlowConnections.tsx`, `tests/workflow.test.ts`                                                          |
| Zagnieżdżone operatory | Wszystkie istniejące operatory, argumenty z typowanymi literałami/odwołaniami/wyrażeniami; operator może zastąpić argument lub zostać upuszczony na planszę                                                         | `shared/operators.ts`, `ExpressionBlock.tsx`, `tests/operators.test.ts`                                  |
| Porównania             | Jeden ogólny klocek porównania, typ jako operator argumentu, bez oddzielnego klocka „sprawdź typ”; porównywanie dowolnych wartości JSON                                                                             | `NodeInspector.tsx`, test przeglądarkowy `one comparison handles values and types through its arguments` |
| Zmienne                | Osobne klocki, parametry i środowisko; przeciąganie na argumenty i pola tekstowe URL/query/header/path; token tekstowy i typowane wiązanie mają różną semantykę                                                     | `features/variables`, `shared/variables.ts`, `tests/variables.test.ts`                                   |
| Pole odpowiedzi        | Przeszukiwalne drzewo body/status/headers, typ i podgląd, edycja ścieżki; null/false/0 nie oznaczają braku; rozróżnienie próbki i ostatniego wyniku                                                                 | `ResponseBindingPicker.tsx`, `ResponseFieldPicker.tsx`, `responseData.ts`                                |
| Odpowiedź → zmienna    | Utworzenie nowej lub ustawienie istniejącej zmiennej odwołaniem do pola; drag pola na argument; brak utrwalania migawkowej wartości jako substytutu referencji                                                      | `AssignResponseVariable.tsx`, `variables/operations.ts`                                                  |
| Komponenty             | Ekstrakcja zaznaczenia, jawne publiczne wejścia/wyjścia dowolnego typu, mapowanie granicy, kontrakty/defaulty, edycja grafu, przypięte rewizje, jawna aktualizacja instancji                                        | `ComponentBuilder.tsx`, `componentDraft.ts`, `componentDraft.test.ts`                                    |
| Kopiuj/wklej           | Niezmienna migawka konfiguracji w chwili kopiowania; nowe ID każdej kopii; przepięcie wewnętrznych referencji także w wyrażeniach i IF; zewnętrzne referencje tylko do istniejących źródeł; blokada drugiego Startu | `graph/operations.test.ts`, ostatni test przeglądarkowy kopiowania                                       |
| Historia               | Undo/redo obejmuje całą zmianę i zależne odwołania cleanup; drag jest jedną operacją, pisanie łączone w sesje edycji; odrębne konteksty main/cleanup/komponent                                                      | Historia w `Workspace.tsx`                                                                               |
| Uruchamianie           | Symulacja/live, pełny scenariusz/do klocka/pojedynczy klocek, anulowanie; wykonanie nie blokuje edytora; wynik wiązany ze snapshotem wejściowym                                                                     | `Workspace.tsx`, `tests/runner.test.ts`, `tests/runner-api.test.ts`                                      |
| Raport                 | Statusy, czas, request/response, zagnieżdżone wyniki komponentów, main/cleanup, wybór kroku prowadzi do klocka; informacja o nieaktualnym wyniku po edycji                                                          | `RunPanel.tsx`, `redact.ts`                                                                              |
| Rozmiar raportu        | Przeciąganie separatora i obsługa klawiaturą; ograniczenia po zmianie okna; zwinięcie zachowuje wysokość                                                                                                            | `ResizableRunPanel.tsx`, test przeglądarkowy resize                                                      |
| Sekrety                | Nazwy widoczne w palecie, wartości wyłącznie w warstwie sekretów; brak wartości w eksportach, schowku klocków, raportach i diagnostyce                                                                              | `EnvironmentEditor.tsx`, `storage.ts`, `redact.test.ts`                                                  |

Obecne 14 testów przeglądarkowych są specyfikacją scenariuszy użytkownika, nie testami dającymi się uruchomić bez zmian przeciwko egui. Trzeba odtworzyć ich działania w natywnym harnessie, zachowując istniejące testy jako referencję zachowania podczas migracji.

## Stan aplikacji i komendy

Podzielić stan na cztery jawne części:

1. `WorkspaceDocument`: model domenowy, projekty i wersjonowane komponenty. Serializowalny, bez egui ID, uchwytów runtime i sekretów. Wersja formatu oraz migracje należą do warstwy modelu/persistencji.
2. `EditorSession`: kontekst dokumentu (projekt/scenariusz/faza/draft), zaznaczenia, kamera planszy, układ paneli, drzewo otwartych pól, historia i schowek grafu. Układ może być zapisany osobno od dokumentu.
3. `InteractionState`: aktywny drag, początek połączenia, modal, fokus pola, robocze bufory JSON, komunikaty błędów. Ten stan nie trafia do eksportu projektu.
4. `RunViewState`: run ID, fingerprint/snapshot dokumentu, postęp, ograniczone rozmiarem wyniki, aktywny krok, stan anulowania. Nie modyfikuje grafu przy nadejściu zdarzeń.

Komponenty egui zbierają intencje (`AddNode`, `ConnectData`, `ConnectControl`, `SetBinding`, `ReplaceExpression`, `PasteFragment`, `CommitComponentRevision`), a wspólny dispatcher waliduje i stosuje transakcję. Renderer nie edytuje bezpośrednio kolekcji grafu. Zapobiega to różnym regułom łączenia w palecie, inspektorze i portach.

Komenda zmieniająca model zwraca nowy stan lub konkretną diagnozę; nie zostawia połowy połączenia. Historia przechowuje transakcje domenowe lub ograniczone rozmiarem snapshoty, a nie stan egui. ID węzłów/rewizji są trwałe; ID widżetów budować z kontekstu i trwałego ID oraz ścieżki argumentu, nigdy z pozycji na liście.

Drag przesuwa podgląd pozycji; zatwierdzenie po zwolnieniu zapisuje jedną komendę. Escape przywraca pozycję początkową. Tekst JSON może być przejściowo niepoprawny: utrzymać bufor, pokazać błąd i nie nadpisywać poprawnego dokumentu nieparsowalnym tekstem.

Runtime pracuje poza wątkiem renderowania. UI przyjmuje zdarzenia z kanału o ograniczonej pojemności i ma budżet obsługi na klatkę. Zdarzenie musi zawierać run ID, aby spóźnione odpowiedzi nie nadpisały nowego uruchomienia. Anulowanie ma własny token; zamknięcie okna nie może czekać na wiszący request na wątku UI. Interfejs oddzielnie sygnalizuje trwający cleanup.

## Klawiatura, fokus i przeciąganie

- Osobny router skrótów: modal → aktywne pole tekstowe → plansza → aplikacja. Ctrl na Linux/Windows; reguły nie mogą być zależne od układu polskiej lub angielskiej klawiatury. Plansza nie przechwytuje kopiuj/wklej/undo, gdy użytkownik edytuje tekst. Escape zamyka wyłącznie najwyższy aktywny kontekst i przywraca poprzedni fokus.
- Schowek grafu to typowany fragment domenowy z migawką; schowek systemowy służy też zwykłemu tekstowi. W pierwszym wydaniu zachować aktualne ograniczenie wklejania grafu do tego samego kontekstu. Nie wprowadzać niejawnego importu referencji między projektami.
- Payload DnD to enum: klocek, operator, odwołanie do zmiennej lub pola odpowiedzi. Nie przenosi wartości sekretów. Cel sam wybiera semantykę: typowane wiązanie argumentu lub interpolacja w tekście. Dla pól tekstowych trzeba świadomie obsłużyć miejsce kursora, zaznaczenie i podgląd przed upuszczeniem.
- Każda operacja przeciągania ma odpowiednik przez przycisk/menu/selekcję. Dla połączeń zachować formularz wyboru następnego kroku i źródła argumentu; sam port na rysowanym grafie nie wystarcza.
- Przyciski/porty wymagają stałych nazw, widocznego fokusu i wystarczającego obszaru trafienia. Rozróżnienie sterowanie/dane i Tak/Nie opiera się także na podpisach/kształcie, nie tylko kolorze.

## Implementacja planszy

**Decyzja architektoniczna:** semantyka grafu i reguły komend muszą być niezależne od biblioteki rysującej. Wybór gotowego widgetu grafowego albo własnego canvasu nie może zmienić modelu projektu ani wymagać skopiowania runnera do UI.

| Wariant                                  | Korzyść                                                     | Warunek dopuszczenia                                                                                                                                            |
| ---------------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Własny canvas nad egui Painter + widżety | Pełna kontrola podpisów, portów, nested input i fokusowania | Własne hit-testy, wybór, przewody, zoom/pan, clipping i semantyka dostępności mają testy oraz właściciela utrzymania                                            |
| Gotowy widget grafu egui                 | Mniej mechaniki rysowania do utrzymania                     | Osobna weryfikacja licencji, aktywności projektu, zgodności z przypiętym egui i wsparcia portów Tak/Nie, dynamicznych wejść, kontroli fokusu oraz stabilnych ID |

Nie wybrano i nie zatwierdzono biblioteki grafowej. Decyzję podjąć po krótkiej implementacji weryfikacyjnej interakcji na docelowej wersji egui; jej celem jest sprawdzenie zależności, nie ograniczenie zakresu produktu.

Plansza potrzebuje jednego jawnego przekształcenia world↔screen; porty i hit-testy używają tego samego transformu. Zoom kotwiczy się pod kursorem, dopasowanie obejmuje zaznaczenie lub cały graf. Rysować przewody pod węzłami i uwzględnić obszar clip panelu. Złożone edytory tekstowe pokazywać w inspektorze lub w trybie edycji o czytelnej skali; nie zakładać, że dowolnie skalowany niestandardowy rysunek zapewni poprawny tekst, IME i dostępność.

Geometria portów jest wynikiem layoutu, nie powinna trafiać do modelu. Zmiana liczby argumentów lub zagnieżdżenia musi odświeżać geometrię bez skoków kamery. Dla dużych grafów stosować odrzucanie niewidocznych elementów, cache geometrii i ograniczoną liczbę pomiarów; testować także przewód przechodzący przez viewport, gdy oba końce są poza nim.

## Natywne ryzyka wymagające walidacji

Dokumentacja eframe opisuje funkcje `accesskit`, `clipboard`, `wayland`, `x11` i `persistence`. Są to możliwości integracji, a nie dowód poprawnej obsługi konkretnej planszy. Przypiąć wersje zgodne z wybranym toolchainem i sprawdzić macierz platform przed deklaracją wsparcia. Źródło: [dokumentacja eframe](https://docs.rs/eframe/latest/eframe/).

1. Czytnik ekranu: standardowe kontrolki i każdy niestandardowy port/węzeł muszą tworzyć użyteczną strukturę, dać się wybrać bez myszy i ogłosić błąd. Samo włączenie AccessKit tego nie gwarantuje. Alternatywna lista kroków i formularz połączeń to część projektu UI.
2. Linux Wayland/X11 oraz Windows, HiDPI i dwa monitory: drag nie gubi stanu poza panelem; clipboard/IME działa; zmiana skali nie przestawia klocków; separator ma czytelną klawiaturową alternatywę. Sprawdzić skróty i polskie znaki na obu systemach, a czytnik ekranu na każdym wspieranym backendzie osobno.
3. Duże odpowiedzi: drzewo JSON wyszukiwane/rozwijane z limitem pracy i wirtualizacją; wielomegabajtowy wynik nie może zatrzymać renderowania. Podgląd skrócony musi jawnie wskazywać skrócenie.
4. Restart/awaria: dokument zapisywany transakcyjnie poza renderowaniem, błędy zapisu widoczne, konflikt zewnętrznej zmiany i odzyskiwanie po awarii mają zdefiniowany UX. Nie polegać wyłącznie na autosave egui.
5. Dane wrażliwe: wyłączyć surowy dump stanu w produkcyjnych logach; przejście z typowanych referencji do tekstu nie może odsłonić sekretów. Screenshoty/testy używają jawnych fixture, nie środowiska użytkownika.

## Etapy i warunki odbioru

| Etap                      | Praca UI                                                                                          | Dowód odbioru                                                                                                                                                                                                           |
| ------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A: kontrakt i referencja  | Wyeksportować aktualne projekty/komponenty i scenariusze zachowań; rozpisać ID kontrolek i skróty | Manifest fixture i mapowanie wszystkich wierszy inwentaryzacji na test; walidacja importu nie zmienia referencji, kontraktów i rewizji                                                                                  |
| B: powłoka i edycja       | eframe, układ, inspektor, komendy, fokus, historia, zapis/otwieranie                              | Edycja → undo/redo → restart daje identyczny dokument; niepoprawny JSON nie psuje zapisu; test przerwanego zapisu                                                                                                       |
| C: plansza i przepływ     | Zaznaczanie, kamera, porty danych/sterowania, Start/IF, kopiowanie grup                           | Odtworzenie `graph/operations.test.ts`; paste zachowuje snapshot, wewnętrzne referencje i gałęzie; drugi Start/cykl odrzucone bez częściowej mutacji                                                                    |
| D: argumenty i komponenty | Operatory, zmienne, interpolacja, wybór response, edycja komponentu                               | Odtworzone przypadki 0/false/null, ścieżki tablic/kluczy specjalnych, nested operator i ekstrakcja publicznych argumentów; rewizja nie zmienia istniejących instancji                                                   |
| E: wykonanie i raport     | Zdarzenia runtime, tryby uruchomień, anulowanie, cleanup, raport/drzewo                           | Fixture HTTP potwierdza ceny → koszyk → produkty; niewybrana gałąź nie robi requestu; UI reaguje podczas wolnego HTTP; stary run nie nadpisuje nowego; resize działa myszą i klawiaturą                                 |
| F: wydanie                | Wydajność, dostępność, pakowanie i migracja danych                                                | Native smoke test na Linux Wayland/X11 i Windows, pełna ścieżka bez myszy, IME/clipboard/DPI/czytnik ekranu na obu systemach, raport wydajności na ustalonych maszynach; czytelny błąd GPU/plików i poprawne zamknięcie |

Proponowane budżety do zatwierdzenia na mierzalnej konfiguracji: interakcja planszy z 500 węzłami i 1000 przewodami bez blokady >100 ms, p95 klatki podczas pan/zoom ≤33 ms; przyjęcie anulowania przez UI ≤100 ms, niezależnie od czasu zakończenia HTTP/cleanup. To kryteria do zmierzenia, nie obecnie potwierdzone parametry aplikacji.

Nie przenosić automatyzacji przez samą analizę screenshotów. Reguły komend testować bez GPU; dla egui stworzyć harness z deterministycznymi klatkami i wejściami użytkownika oraz sprawdzaniem stanu/fokusu. Natywne clipboard/IME/AccessKit/GPU nadal wymagają testów integracyjnych na rzeczywistym backendzie. Zrzuty ekranu wspierają kontrolę layoutu przy ustalonym foncie, skali i rendererze.
