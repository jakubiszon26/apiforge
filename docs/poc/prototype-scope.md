# API Forge — zakres prototypu v1

Status: zakres zaakceptowany do implementacji. Development rozpoczęty na polecenie użytkownika; aktualne działanie i ograniczenia opisuje README.md. Makieta w `design-preview/` pozostaje wcześniejszym eksperymentem interfejsu.

## 1. Cel i granice

Tester manualny buduje i uruchamia scenariusze REST API na grafie. Zespół definiuje swoje requesty, typy danych, komponenty oraz akcje biznesowe. Produkt, koszyk i promocja są wyłącznie przykładem konfiguracji jednego projektu. Silnik i interfejs nie zawierają ich specjalnej obsługi.

Główny przebieg: zdefiniuj request → zapisz jako komponent z argumentami → umieść go na grafie → wybierz pasującą akcję → połącz dane → dodaj sprawdzenie → uruchom → obejrzyj przyczynę wyniku.

Założenia projektowe v1: lokalna aplikacja dla jednego użytkownika, jeden otwarty projekt, trwały zapis lokalny i eksport/import JSON. Interfejs po polsku. Bez wymogu pisania JavaScriptu. Założenia można zmienić po ocenie prototypu; nie wymagają teraz kolejnego wywiadu.

## 2. Dwa etapy prototypu

**P1 — sprawdzenie sposobu projektowania.** Działający edytor: drag and drop, połączenia, konfiguracja requestów, menu akcji, tworzenie własnych komponentów, podgląd odpowiedzi, budowanie asercji. Wyniki pochodzą z jawnie oznaczonych danych przykładowych. To pierwszy punkt oceny przez użytkownika.

**P2 — użyteczny test API.** Ten sam zapisany graf wykonuje prawdziwe requesty przez lokalny backend. Dodajemy raport, wykonanie części scenariusza, obsługę błędów i sprzątanie. Dopiero P2 spełnia kryteria kompletnego prototypu v1. Symulacja nigdy nie udaje rzeczywistego uruchomienia.

## 3. Edytor

- Lewa biblioteka: operacje wbudowane oraz komponenty bieżącego projektu.
- Środek: przesuwany i skalowany graf. Przeciąganie klocków z biblioteki, przenoszenie, łączenie, usuwanie, duplikowanie, zaznaczanie wielu klocków, cofanie/ponawianie zmian grafu.
- Na klocku: nazwa, krótki opis działania, wybrane przez autora istotne argumenty, nazwane porty i stan wykonania. Dla requestu: metoda i wzorzec ścieżki. Argument `id` jest zwykłym argumentem zdefiniowanym przez autora.
- Prawy panel: pełna konfiguracja zaznaczonego klocka lub wnętrze komponentu.
- Dolny panel: przebieg wykonania, wejścia, wyjścia, request, response i błędy.
- Widoczny przycisk „Dodaj akcję” daje te same możliwości co prawy przycisk myszy. Podstawowy scenariusz da się zbudować klikaniem, bez przeciągania.
- W v1 użytkownik może wybrać jeden jawnie podany zestaw argumentów scenariusza. Tabele przypadków i pętle są kolejnym rozszerzeniem.

### Menu kontekstowe i tworzenie połączeń

Prawy przycisk na komponencie lub przeciągnięcie z portu w puste miejsce pokazuje akcje zgodne z wybranym wyjściem. Przy wielu wyjściach użytkownik wskazuje konkretne wyjście.

Akcję można przeciągnąć z menu na graf lub kliknąć, aby dodać ją obok źródła. Tworzony jest klocek i połączenie. Zgodne wejście przypisuje się automatycznie; jeśli pasuje kilka wejść, użytkownik wybiera jedno. Pozostałe wymagane argumenty są widocznie oznaczone.

Menu łączy uniwersalne operacje i akcje projektu. Autor komponentu określa, przy jakim typie wyjścia komponent ma być proponowany oraz które jego wejście ma być zasilane. Nie ma zaszytej wiedzy o nazwach endpointów ani znaczeniu pól.

## 4. Własne komponenty i kontrakty danych

Komponent może opakowywać pojedynczy request albo graf kilku operacji. Zaznaczenie fragmentu grafu i „Zapisz jako komponent” otwiera edytor publicznych argumentów, wyjść i pól pokazywanych na klocku. Przecinające granicę połączenia trzeba jawnie przypisać do publicznych portów.

Definicja zawiera: stabilne ID, nazwę, opis, rewizję, argumenty z typami i domyślnymi wartościami, graf wewnętrzny, mapowanie wyjść oraz opcjonalne przypisania do menu akcji. Edytor umożliwia zmianę wnętrza i zapis nowej rewizji. Instancja przypina konkretną rewizję; aktualizację stosuje się jawnie do wybranych instancji. Zapisany test nie zmienia zachowania po cichej aktualizacji biblioteki.

Typy bazowe: tekst, liczba, boolean, obiekt, lista, dowolny JSON. Zespół może nazwać kontrakt obiektu, np. `User` albo `Product`, i przypisać mu opis pól. W v1 zgodność akcji wynika z typu bazowego lub identyfikatora kontraktu projektu; nie implementujemy pełnego systemu zgodności JSON Schema.

Struktura odpowiedzi pochodzi z ręcznego opisu lub zapisanej przykładowej odpowiedzi. Struktura rozpoznana z pojedynczej odpowiedzi jest oznaczona jako podpowiedź. Obserwowane pole nie staje się automatycznie wymaganiem testowym. Import OpenAPI odkładamy.

Przy tworzeniu komponentu zagnieżdżonego można korzystać z innych zapisanych komponentów. Zależności cykliczne i rekurencyjne są niedozwolone.

## 5. Wbudowane klocki

| Klocek               | Zakres v1                                                                                                                                              |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Request HTTP         | Metoda, URL lub base URL + ścieżka, argumenty ścieżki, query, nagłówki, body JSON, timeout. Wyjście: status, nagłówki, body JSON lub tekst, czas.      |
| Wybierz dane         | Wybór z drzewa JSON lub prosta ścieżka: pole, zagnieżdżenie, indeks listy. Rozróżnia brak pola, `null`, pusty tekst i zero.                            |
| Zbuduj dane          | Obiekt/lista składana z literałów i wartości wcześniejszych klocków; do przygotowania body i argumentów.                                               |
| Formuła              | Dodawanie, odejmowanie, mnożenie, dzielenie, zaokrąglenie i składanie tekstu. Argumenty jako wartości lub powiązania. Operacje można łączyć na grafie. |
| Porównanie / asercja | Równość, nierówność, porównanie liczb, tolerancja liczbowa, istnienie, typ, niepuste, zawieranie. Wynik zawiera wartość oczekiwaną i otrzymaną.        |
| Warunek wejściowy    | Te same proste predykaty, łączone przez „wszystkie” lub „dowolny”. Niespełnienie oznacza niewłaściwe dane i blokuje zależne kroki.                     |

Formuły są zapisem dozwolonych operacji, bez `eval` i wykonywania dowolnego JavaScriptu. W obliczeniach cen tester jawnie wskazuje sposób zaokrąglania i tolerancję; silnik nie zakłada żadnej waluty ani reguł biznesowych.

## 6. Powiązania i wykonanie

- Argument jest literałem, parametrem scenariusza, zmienną środowiska albo odwołaniem `nodeId + outputPort + path`.
- Połączenie przenosi wskazane dane oraz ustanawia zależność wykonania. Podpięty argument pokazuje źródło zamiast edytowalnej wartości literalnej.
- Wybór danych wcześniejszego klocka tworzy jawne powiązanie i zależność. Nie używamy niewidocznego, nadpisywanego globalnego „bieżącego response”.
- Są również połączenia „Dalej”, gdy potrzebna jest kolejność bez przekazania danych. Ręczny Start wyznacza punkt wejścia; IF / ELSE wybiera gałąź Tak albo Nie. Nieaktywne kroki są pomijane. Starsze grafy bez Startu uruchamiają wszystkie korzenie.
- Graf jest acykliczny. Klocek oczekuje na wszystkie wymagane wejścia. Do jednego wejścia przypisujemy jedno źródło. Połączenie dwóch gałęzi wymaga oddzielnych nazwanych wejść.
- v1 wykonuje gotowe kroki sekwencyjnie w stabilnej kolejności; równoległość jest późniejszym rozszerzeniem.
- Każdy przebieg używa utrwalonej kopii grafu, rewizji komponentów i parametrów. Edycja grafu nie zmienia aktywnego przebiegu.

### Błędy i kontynuacja

Rozróżniamy: niespełnioną asercję, błąd wykonania, niewłaściwe dane wejściowe, krok zablokowany, pominięty i anulowany. Raport nie oznacza zablokowanych sprawdzeń jako poprawnych.

Odpowiedź HTTP 4xx/5xx nadal jest odpowiedzią do testowania; status można jawnie sprawdzić asercją. Timeout, brak połączenia lub niemożliwe mapowanie danych są błędami wykonania. Brak pola można sprawdzić asercją, ale nie można bez ostrzeżenia zamienić go na zero w formule.

Ustawienie scenariusza: zatrzymaj po błędzie albo kontynuuj możliwe kroki. W drugim trybie asercja z wynikiem negatywnym zachowuje swoje dane, a kolejne sprawdzenia mogą działać. Kroki wymagające nieistniejących wyjść pozostają zablokowane; niezależne gałęzie mogą działać. Warunek wejściowy zawsze blokuje swoje zależne kroki.

### Wykonanie częściowe i sprzątanie

- „Uruchom test” oraz „uruchom do klocka” wykonują świeży przebieg; drugi tryb uruchamia wyłącznie przodków i wskazany klocek.
- „Wykonaj ten krok” używa aktualnych literałów albo jawnie wybranych, zapisanych wejść poprzedniego przebiegu. UI pokazuje ich pochodzenie i czas. Taki wynik jest diagnostyczny, nie zalicza pełnego testu.
- Operacje HTTP nie są automatycznie ponawiane. Powtórzenie kroku może ponownie zmienić dane; kliknięcie wykonania jest świadomym wyborem użytkownika.
- Osobna sekcja „Po teście” zawiera zwykły graf sprzątania, uruchamiany także po błędzie i kontrolowanym anulowaniu. Brak utworzonego ID daje jawne pominięcie odpowiedniego kroku. Awaria procesu nie gwarantuje sprzątania.
- Błąd sprzątania jest pokazany osobno i nie nadpisuje pierwotnego błędu testu. Anulowanie nie cofa requestów już wykonanych przez zewnętrzny serwer.

## 7. Asercje z odpowiedzi i raport

Kliknięcie pola w drzewie odpowiedzi daje „Sprawdź…”, „Użyj jako argument” oraz „Wystaw jako wyjście komponentu”. Dodanie asercji uzupełnia ścieżkę i źródło. Dla pola liczbowego można wybrać wartość, inne pole lub wynik formuły jako oczekiwanie.

Wynik pokazuje status każdego kroku, dane wejściowe, odpowiedź, czas oraz szczegóły asercji. Błąd własnego komponentu wskazuje wewnętrzny krok. Dostępne są pełne request/response z maskowaniem pól oznaczonych jako sekrety i nagłówków autoryzacji.

Porównanie dwóch endpointów potwierdza ich spójność; niezależna wartość lub formuła stanowi osobny wzorzec. UI nie przedstawia samej zgodności endpointów jako dowodu poprawności reguły biznesowej.

## 8. Środowisko i zapis

Projekt przechowuje scenariusze, komponenty, kontrakty i przykładowe odpowiedzi. Import i eksport mają wersję formatu i walidację. Nieznana wersja lub brak definicji komponentu daje czytelny błąd. Dane projektu przetrwają odświeżenie strony; historia przebiegów może być sesyjna w v1, z ręcznym eksportem raportu.

Środowisko ma base URL, zwykłe zmienne i wartości tajne. Obsługujemy nagłówki, w tym Bearer/API key. Sekrety wpisane na czas sesji nie trafiają do eksportu projektu ani raportów; pełne response mogą zawierać dane wrażliwe, dlatego eksport raportu jest jawną akcją. OAuth i automatyczne przepływy logowania są poza v1; projekt może przygotować zwykły request logowania i powiązać jego wynik.

Requesty wykonuje lokalny backend Express, aby klient nie zależał od CORS testowanego API. Runner jest dostępny lokalnie, z walidacją przychodzących żądań, ograniczeniami czasu i rozmiaru oraz kontrolą źródła wywołania. Sieć lokalna i localhost pozostają dostępnymi celami testów. Publiczny, wieloużytkownikowy hosting wymaga osobnego projektu zabezpieczeń i nie należy do v1.

## 9. Zakres odłożony

Pętle i tabele wariantów, równoległe wykonanie, import OpenAPI, generowanie testów przez AI, dowolne skrypty JS, testy wizualne przeglądarki, CI i harmonogramy, współpraca wielu osób, chmurowe środowiska, marketplace, automatyczne ponawianie i oczekiwanie na eventual consistency. Projekt formatu umożliwia późniejsze dodanie tych funkcji bez implementowania ich teraz.

## 10. Odbiór v1

1. Tester definiuje request na dowolny endpoint, dowolne nazwane argumenty i strukturę odpowiedzi bez edycji kodu aplikacji.
2. Zapisuje komponent i umieszcza go na grafie; wybrane argumenty edytuje bezpośrednio na klocku.
3. Dodaje asercję z pola odpowiedzi przez kliknięcie; źródło danych i połączenie powstają automatycznie.
4. Zapisuje kilka kroków jako własną akcję z argumentami i wyjściami; akcja pojawia się przy zgodnym typie danych.
5. Przeciąga akcję na graf oraz wykonuje równoważną operację samymi kliknięciami.
6. Uruchamia prawdziwy scenariusz: odczyt → utworzenie danych → kolejny odczyt → porównanie → sprzątanie.
7. Rozpoznaje błąd asercji, błąd sieci i niewłaściwe dane; polityka kontynuacji działa zgodnie z opisem.
8. Zapisuje, odświeża, eksportuje i importuje projekt bez utraty powiązań i rewizji.
9. Dwa projekty demonstracyjne z różnych domen korzystają z tych samych operacji, a różnią się wyłącznie definicjami projektu. Żaden warunek w silniku ani UI nie sprawdza nazwy „Produkt” czy konkretnej ścieżki endpointa.
10. Symulacje, wcześniejsze odpowiedzi i rzeczywiste przebiegi są wyraźnie rozróżnione.
