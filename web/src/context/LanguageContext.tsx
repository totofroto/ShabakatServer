import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
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

type LanguageContextValue = {
  lang: AppLang;
  isRtl: boolean;
  setLang: (lang: AppLang) => void;
  toggleLang: () => void;
  dict: Dictionary;
};

const LS_KEY = "shabakat_lang";

function readLang(): AppLang {
  try {
    const v = localStorage.getItem(LS_KEY);
    if (v === "ar" || v === "en" || v === "de") return v;
  } catch {}
  return "en";
}

const LanguageContext = createContext<LanguageContextValue | undefined>(
  undefined,
);

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<AppLang>(readLang);

  const setLang = useCallback((next: AppLang) => {
    try {
      localStorage.setItem(LS_KEY, next);
    } catch {}
    setLangState(next);
  }, []);

  const toggleLang = useCallback(() => {
    setLangState((prev) => {
      let next: AppLang = "en";
      if (prev === "en") next = "ar";
      else if (prev === "ar") next = "de";
      else next = "en";
      
      try {
        localStorage.setItem(LS_KEY, next);
      } catch {}
      return next;
    });
  }, []);

  const isRtl = lang === "ar";
  const dict = dictionaries[lang] || enDict;

  // Keep the HTML `dir` attribute in sync so native browser RTL rendering
  // (text alignment, scrollbars, flex rows, etc.) works without extra CSS.
  useEffect(() => {
    document.documentElement.dir = isRtl ? "rtl" : "ltr";
    document.documentElement.lang = lang;
  }, [isRtl, lang]);

  return (
    <LanguageContext.Provider value={{ lang, isRtl, setLang, toggleLang, dict }}>
      {children}
    </LanguageContext.Provider>
  );
}

export function useLanguage(): LanguageContextValue {
  const ctx = useContext(LanguageContext);
  if (!ctx) {
    throw new Error("useLanguage must be used within LanguageProvider");
  }
  return ctx;
}
