export async function decodeJsonResponse<T>(response: Response): Promise<T> {
  const text = await response.text();
  if (!text.trim())
    throw new Error(
      `Runner zwrócił pustą odpowiedź (HTTP ${response.status}). Sprawdź, czy lokalne API Forge działa na porcie 4318.`,
    );
  if (!response.headers.get('content-type')?.toLowerCase().includes('json'))
    throw new Error(
      `Runner zwrócił odpowiedź inną niż JSON (HTTP ${response.status}). Pod tym adresem może działać inna aplikacja. Uruchom API Forge poleceniem npm run dev — runner korzysta z portu 4318.`,
    );
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error(
      `Runner zwrócił niepoprawny JSON (HTTP ${response.status}). Spróbuj ponownie; jeśli błąd się powtarza, uruchom ponownie lokalny serwer.`,
    );
  }
  if (!response.ok) {
    const error = data && typeof data === 'object' && 'error' in data ? data.error : undefined;
    throw new Error(
      typeof error === 'string' ? error : `Błąd lokalnego API Forge (HTTP ${response.status}).`,
    );
  }
  return data as T;
}
export async function jsonRequest<T>(url: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(url, init);
  } catch {
    throw new Error(
      'Nie można połączyć z lokalnym runnerem. Uruchom API Forge poleceniem npm run dev.',
    );
  }
  return decodeJsonResponse<T>(response);
}
