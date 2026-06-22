import enDict from "../locales/en.json";
import deDict from "../locales/de.json";
import arDict from "../locales/ar.json";

export type AppLang = "en" | "ar" | "de";
type Dictionary = typeof enDict;

const dictionaries: Record<AppLang, Dictionary> = {
  en: enDict,
  de: deDict,
  ar: arDict,
};

/**
 * Hardened localization helper.
 * Returns the translation for the given key in the specified language.
 * Falls back to English, then to the key itself if not found.
 */
export function t(lang: AppLang, key: string, dictOverride?: any): string {
  const dict = dictOverride || dictionaries[lang] || dictionaries.en;
  
  // Try direct lookup
  if (dict && typeof dict[key] === "string") {
    return dict[key];
  }

  // Handle nested keys (e.g. "tabs.home")
  if (key?.includes(".")) {
    const parts = key.split(".");
    let current: any = dict;
    for (const part of parts) {
      if (current && typeof current === "object" && part in current) {
        current = current[part];
      } else {
        current = undefined;
        break;
      }
    }
    if (typeof current === "string") return current;
  }

  // Fallback to English dictionary if not already using it
  if (lang !== "en") {
    const enDictRef = dictionaries.en;
    if (enDictRef && typeof (enDictRef as any)[key] === "string") {
      return (enDictRef as any)[key];
    }
  }

  return key;
}
