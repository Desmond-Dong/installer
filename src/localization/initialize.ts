import { isTauri } from "@tauri-apps/api/core";
import { locale } from "@tauri-apps/plugin-os";
import { getLanguage, localize, setLanguage } from "./localize.js";

/** [China mirror] A manual language choice, made on the welcome screen. */
const LANGUAGE_STORAGE_KEY = "hai.language";

export function readStoredLanguage(): string | null {
  try {
    return localStorage.getItem(LANGUAGE_STORAGE_KEY);
  } catch {
    // Storage may be unavailable; automatic detection still works.
  }
  return null;
}

export function setStoredLanguage(language: string | null): void {
  try {
    if (language) {
      localStorage.setItem(LANGUAGE_STORAGE_KEY, language);
    } else {
      localStorage.removeItem(LANGUAGE_STORAGE_KEY);
    }
  } catch {
    // Storage may be unavailable; the choice simply won't persist.
  }
}

export async function detectLanguage(
  browserLanguages: readonly string[],
  readNativeLocale?: () => Promise<string | null>
): Promise<void> {
  let nativeLocale: string | null = null;
  if (readNativeLocale) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      nativeLocale = await Promise.race([
        readNativeLocale(),
        new Promise<null>((resolve) => {
          timer = setTimeout(() => resolve(null), 1000);
        }),
      ]);
    } catch {
      // Locale discovery is optional; the English UI must still start.
    } finally {
      clearTimeout(timer);
    }
  }
  setLanguage(
    nativeLocale ? [nativeLocale, ...browserLanguages] : browserLanguages
  );
}

export async function initializeLocalization(): Promise<void> {
  const stored = readStoredLanguage();
  await detectLanguage(
    stored ? [stored, ...navigator.languages] : navigator.languages,
    isTauri() ? locale : undefined
  );
  document.documentElement.lang = getLanguage();
  document.title = localize("app.title");
}
