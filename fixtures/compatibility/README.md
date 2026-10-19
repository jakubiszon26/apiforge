# Fixture kompatybilności projektów i workspace (v1)

Ten katalog przechowuje deterministyczne pliki referencyjne (fixture) dla formatu projektu i przestrzeni roboczej APIForge w wersji `v1`:

- `projects/shop.json`
- `projects/tickets.json`
- `projects/blocks.json`
- `projects/workflow.json`
- `workspace-v1.json`
- `manifest.json` (sumy kontrolne `sha256` wygenerowanych plików)

## Po co istnieją te fixture

Fixture służą do pilnowania kompatybilności wstecznej schematu zapisu projektów (`Project` v1) oraz eksportu całej przestrzeni roboczej (`workspace-v1.json`). Pozwalają zweryfikować, że zmiany w generatorach projektów demonstracyjnych, walidacji grafu (`validateProject`) lub strukturze danych nie zmieniają nieświadomie formatu serializacji ani referencji pomiędzy węzłami, scenariuszami, kontraktami i komponentami.

Wszystkie losowe identyfikatory (`project-*`, `scenario-*`, `node-*`, `order-*`) są podczas eksportu deterministycznie mapowane wg kolejności przejścia drzewa na stabilne wartości (`prefix-00000001`, `prefix-00000002`, ...), a wartości zmiennych oznaczonych w `environment.secretNames` są usuwane z `environment.variables`.

## Zakres (brak testowania wykonania w Rust)

Te fixture sprawdzają wyłącznie strukturę danych, walidację modelu projektu oraz deterministyczną serializację JSON. **Nie testują wykonania scenariuszy ani silnika uruchomieniowego Rust** (nie uruchamiają zapytań HTTP, asercji runtime ani komend edytora Rust).

## Generowanie i aktualizacja fixture

Aby wygenerować lub zaktualizować cały zestaw plików na dysku:

```bash
node --import tsx scripts/export-compatibility-fixtures.ts
```

## Sprawdzanie spójności (np. w CI)

Aby porównać wygenerowane w pamięci bajty z plikami zapisanymi na dysku bez modyfikowania żadnych plików:

```bash
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

Polecenie zakończy się kodem:

- `0` – gdy wszystkie pliki na dysku są identyczne bajt po bajcie z wygenerowanym zestawem,
- `1` – gdy brakuje pliku, występują różnice w zawartości lub walidacja `validateProject` zwróci błędy,
- `2` – gdy przekazano nieobsługiwane argumenty linii poleceń.
