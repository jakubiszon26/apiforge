# Praca nad wersją Rust

Pierwszy przyrost obejmuje importer v1 i natywny podgląd dokumentów, z połączeniami danych oraz kolejności. Edytowanie, utrwalanie zmian i wykonywanie scenariuszy są dalszymi etapami [planu migracji](../../RUST_MIGRATION.md). Dotychczasową aplikację uruchamia się nadal przez `npm run dev`.

## Zależności i uruchomienie

Rust 1.99.0 z rustfmt i clippy; wersję określa `rust-toolchain.toml`, zależności `Cargo.lock`. Workspace tworzą na razie dwa rzeczywiste moduły: `apiforge-core` i `apiforge-desktop`. Kolejne crate'y dochodzą wraz z implementacją ich odpowiedzialności, bez pustych atrap.

Na Linuksie potrzebna jest działająca sesja X11 albo Wayland, sterownik zgodny z backendem WGPU, biblioteki systemowe do kompilacji oraz portal plików dla dialogu Open. Lista pakietów Ubuntu znajduje się w [workflow CI](../../.github/workflows/rust.yml). Wymagania minimalnych wersji dystrybucji i Windows oraz pakowanie nadal wymagają odbioru.

```bash
cargo run -p apiforge-desktop --locked -- --project fixtures/compatibility/workspace-v1.json
```

Przycisk **Otwórz projekt / workspace…** importuje istniejący plik JSON. Import i dialog działają poza wątkiem renderowania. Błąd albo anulowanie wyboru nie zastępuje otwartego dokumentu. Obecny podgląd nie nadpisuje plików wejściowych.

Bez środowiska graficznego można sprawdzić plik:

```bash
cargo run -p apiforge-desktop --locked -- --check fixtures/compatibility/workspace-v1.json
```

Tryb sprawdzenia zwraca 0 przy poprawnym imporcie, 1 przy błędzie dokumentu/odczytu, 2 przy błędnych argumentach. Wartości pól i sekretów nie są wypisywane w diagnostyce parsera. Import ma limit 16 MiB i nie wykonuje połączeń HTTP.

## Weryfikacja

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --release -p apiforge-desktop --locked
```

Testy obejmują round-trip projektów i workspace, brak/null, Unicode i kolejność argumentów, duplikaty kluczy, nieznane pola, limity, referencje, Start/IF, cykle, rewizje/rekurencję oraz zachowanie otwartego dokumentu po nieudanym imporcie. Testy egui wykonują layout wszystkich projektów i obu faz bez GPU; nie są testami całego natywnego backendu.

Dodatkowy test startu rzeczywistego okna otwiera dokument, rysuje co najmniej trzy klatki i zamyka aplikację. Wymaga sesji graficznej:

```bash
cargo run -p apiforge-desktop --locked -- --smoke-test fixtures/compatibility/workspace-v1.json
```

Test nie potwierdza obsługi schowka, IME, dostępności, dialogu wyboru pliku ani kompletności edycji. Windows musi zostać zweryfikowany na natywnym runnerze; lokalne testy Linux tego nie zastępują.

Korpus porównawczy jest generowany ze źródeł TypeScript:

```bash
node --import tsx scripts/export-compatibility-fixtures.ts
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

Nie formatuj ręcznie wygenerowanych JSON-ów: ich bajty i SHA256 muszą zgadzać się z generatorem. Testy zgodności porównują wartości liczbowe JS (np. `100` i `100.0` mają ten sam sens), obecność pól, kolejność kolekcji istotnych dla argumentów i całe konfiguracje. Identyfikatory demonstracji są ustabilizowane, bez maskowania różnic w konfiguracji.

## Delegacja i granice bieżącej implementacji

Argon przez `agy` przygotował workflow i skrypt korpusu; zostały przejrzane i poprawione przez koordynatora. Nie otrzymał odpowiedzialności za semantykę silnika, migrację dokumentu ani ochronę sekretów.

Przed wydaniem pozostają: pełny typowany model wykonania i nowe typy ID, evaluator, formalna aktywacja gałęzi, runner async, polityka sekretów i diagnostyki historycznej, SQLite/recovery, komendy edycji/undo, komplet interakcji egui, testy natywne obu OS, podpisywanie i instalatory. Obecny podgląd nie jest zamiennikiem działającego edytora TypeScript.

## Ostatnia weryfikacja lokalna

19.10.2026, Linux x86_64: 14 testów Rust przeszło; `cargo clippy` z `-D warnings`, sprawdzanie formatowania i build release również. Binarka release zaimportowała czteroprojektowy workspace przez `--check` oraz uruchomiła rzeczywiste okno i zakończyła `--smoke-test` kodem 0. Generator przeszedł `--check`, a TypeScript `npm run typecheck`.

Workflow Linux/Windows został dodany, lecz nie uruchomiony z tego środowiska na GitHub Actions. Testy Windows, czytników ekranu i instalatorów pozostają niepotwierdzone. R0 i R1 są rozpoczęte, nie zamknięte; runtime i pozostałe funkcje wymagają dalszych etapów.
