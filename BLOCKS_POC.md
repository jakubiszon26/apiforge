# Plan i PoC: testy z bloków operatorów

## Sposób budowania

Requesty, porównania i komponenty pozostają krokami scenariusza. Każdy ich argument może zawierać wartość, źródło danych albo zagnieżdżony blok operatora. Wynik operatora zasila otaczający argument. Np. Porównanie przyjmuje wynik `i(typ(status) = tekst, długość(status) > 0)` i oczekuje `true`. Połączenia ze źródłami wewnątrz bloków pozostają jawnymi zależnościami grafu.

Jedno Porównanie jest miejscem zaliczenia/niezaliczenia testu. Operatory porównania i logiki tylko zwracają wartość boolean. Nie mnożymy osobnych komponentów „sprawdź typ”, „sprawdź długość” itp.

## Etapy

1. Wspólny katalog operatorów: arytmetyka, losowanie, porównania, i/lub/nie, tekst, modulo, zaokrąglenie i funkcje matematyczne; dodatkowo typ, istnienie, listy i odczyt pola JSON.
2. Serializowalne drzewo wyrażeń w istniejącym modelu Binding; jeden interpreter dla symulacji i rzeczywistego API. Walidacja zagnieżdżeń, typów, referencji i cykli; bez eval.
3. Edytor kolorowych, zagnieżdżonych bloków w gniazdach argumentów. Wybór operatora z grup, dodawanie kliknięciem i przeciągnięciem z palety. Edycja liści przez istniejący edytor wartości i źródeł.
4. Integracja grafu, zapisu/importu, cofania, duplikowania i własnych komponentów. Przykład: odczyt API → złożony warunek → jedno Porównanie.
5. Testy interpretera, błędów typów, referencji w zagnieżdżeniach i pełnego przebiegu w przeglądarce.

## Reguły PoC

Inspiracją jest katalog operatorów Scratch: https://github.com/scratchfoundation/scratch-vm/blob/develop/src/blocks/scratch3_operators.js. Nie kopiujemy niejawnej konwersji typów: null i brak pola nie stają się zerem, tekst nie staje się liczbą, logika przyjmuje boolean. To potrzebne, żeby test API wykrywał błędy kontraktu. Porównanie równości uwzględnia typ i strukturę JSON; zawieranie tekstu uwzględnia wielkość liter. Kąty funkcji trygonometrycznych i numer litery odpowiadają konwencji Scratch (stopnie i indeks od 1). Zaokrąglenie dodatkowo przyjmuje precyzję. Pola i indeksy JSON zachowują dotychczasowe ścieżki i indeksy od 0.

Operatory są skończonym katalogiem, nie kodem JavaScript. Losowanie jest jawne i nie służy jako stabilny wzorzec oczekiwań. Maksymalnie 12 poziomów i 200 bloków w jednym argumencie. Eksport z operatorami wymaga tej lub nowszej wersji aplikacji; starsza aplikacja odrzuci nieznany rodzaj powiązania.

## Poza PoC

Pętle, czekanie/retry, równoległość i pełny edytor stosów poleceń wymagają rozszerzenia modelu wykonania i raportu. To kolejny etap. Obecny PoC dotyczy składania wyrażeń z bloków w rzeczywistych scenariuszach API, bez zmiany silnika na wykonujący dowolne skrypty.

## Rozszerzenie: przepływ wykonania (zrealizowane)

1. Model: ręczny Start, IF z tymi samymi predykatami co Porównanie i krawędzie kolejności z `branch: true/false` (wartości tekstowe). Maksymalnie jeden Start w grafie, bez wejść; graf nadal jest acykliczny.
2. Runner: czeka na zakończenie operacji; uruchamia ścieżkę od Startu i zależne bloki danych, a nieaktywne gałęzie oznacza jako pominięte. IF false jest poprawnym wynikiem kroku, wybierającym ELSE. Dane nie mogą wymusić uruchomienia requestu z pominiętej gałęzi. Starsze grafy bez Startu zachowują poprzednią semantykę.
3. Edytor: większe porty sterowania, podpisane wyjścia Dalej / Tak / Nie, dodawanie połączonego kroku z wyjścia i edycja połączeń istniejących bloków w konfiguracji. Powiązania danych pozostają osobnymi portami.
4. Przykład: ceny produktu → IF status 200 → pusty koszyk → dodanie produktów → porównanie sumy; ELSE raportuje brak cen, sprzątanie usuwa wyłącznie utworzony koszyk.
5. Weryfikacja: lokalne HTTP mierzące kolejność i maksymalną liczbę aktywnych requestów; true/false, dalsze kroki obu gałęzi, scalenie, niedostępne dane, błędy, koszyk i cleanup, zapis/reload oraz łączenie w przeglądarce.
