import { useEffect, useState } from "react";
import {
  Bell,
  LogOut,
  Monitor,
  Radar,
  ShieldCheck,
  Wifi,
  Wrench,
} from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";
import {
  NotificationCenterBell,
  NotificationCenterPanel,
} from "@/components/notification-center-drawer";
import { useLanguage } from "@/context/LanguageContext";
import { useAuth } from "@/context/AuthContext";
import { cn } from "@/lib/utils";

const UI: Record<
  | "appName"
  | "switchToArabic"
  | "switchToEnglish"
  | "switchToGerman"
  | "spatialPosture"
  | "engineCaption"
  | "wifiRequired"
  | "dashboard"
  | "devices"
  | "activity"
  | "alerts"
  | "tools"
  | "settings"
  | "discover"
  | "monitor"
  | "networkInfo"
  | "aboutSystem"
  | "profileName"
  | "profilePartner"
  | "profileOrigin"
  | "profileLocation"
  | "logout"
  | "adminSession",
  { en: string; ar: string; de?: string }
> = {
  appName: { en: "SHABAKAT", ar: "شبكات", de: "SHABAKAT" },
  switchToArabic: { en: "Switch to Arabic", ar: "التبديل إلى العربية", de: "Auf Arabisch umstellen" },
  switchToEnglish: { en: "Switch to English", ar: "التبديل إلى الإنجليزية", de: "Auf Englisch umstellen" },
  switchToGerman: { en: "Switch to German", ar: "التبديل إلى الألمانية", de: "Auf Deutsch umstellen" },
  spatialPosture: { en: "Spatial network posture", ar: "وضع الشبكة المكاني", de: "Räumliche Netzwerkhaltung" },
  engineCaption: {
    en: "Scanner engine · local network posture",
    ar: "محرك الفحص · وضع الشبكة المحلية",
    de: "Scanner-Engine · lokale Netzwerkhaltung",
  },
  wifiRequired: {
    en: "Connect to Wi-Fi to run a local network scan.",
    ar: "التبديل إلى الإنجليزية",
    de: "Mit Wi-Fi verbinden, um einen lokalen Netzwerkscan durchzuführen.",
  },
  dashboard: { en: "Dashboard", ar: "لوحة المعلومات", de: "Dashboard" },
  devices: { en: "Devices", ar: "الأجهزة", de: "Geräte" },
  activity: { en: "Activity", ar: "النشاط", de: "Aktivität" },
  alerts: { en: "Alerts", ar: "التنبيهات", de: "Warnungen" },
  tools: { en: "Tools", ar: "الأدوات", de: "Werkzeuge" },
  settings: { en: "Settings", ar: "الإعدادات", de: "Einstellungen" },
  discover: { en: "Discover", ar: "اكتشاف", de: "Entdecken" },
  monitor: { en: "Monitor", ar: "مراقبة", de: "Überwachen" },
  networkInfo: { en: "Network Info", ar: "شبكة", de: "Netzwerk-Info" },
  aboutSystem: { en: "About System", ar: "عن النظام", de: "Über das System" },
  profileName: { en: "Tareg Shek", ar: "طارق شيك", de: "Tareg Shek" },
  profilePartner: {
    en: "Fatima-Samsmona Allaymona Waddan",
    ar: "فاطمة سمسمونة الليمونة ودان",
    de: "Fatima-Samsmona Allaymona Waddan",
  },
  profileOrigin: { en: "Libya", ar: "ليبيا", de: "Libyen" },
  profileLocation: { en: "Datteln, Germany", ar: "داتلن، ألمانيا", de: "Datteln, Deutschland" },
  logout: { en: "Log out", ar: "تسجيل الخروج", de: "Abmelden" },
  adminSession: { en: "Admin Session", ar: "جلسة المسؤول", de: "Admin-Sitzung" },
};

const sidebarNavigation: Array<{
  key: "dashboard" | "devices" | "activity" | "alerts" | "tools" | "settings";
  to: string;
}> = [
  { key: "dashboard", to: "/" },
  { key: "devices", to: "/devices" },
  { key: "activity", to: "/activity" },
  { key: "alerts", to: "/alerts" },
  { key: "tools", to: "/tools" },
  { key: "settings", to: "/settings" },
];

const tabBarItems: {
  id: string;
  to: string;
  labelKey: "discover" | "devices" | "monitor" | "tools" | "networkInfo";
  Icon: typeof Radar;
  end?: boolean;
}[] = [
  { id: "discover", to: "/", end: true, labelKey: "discover", Icon: Radar },
  { id: "devices", to: "/devices", labelKey: "devices", Icon: Monitor },
  { id: "monitor", to: "/alerts", labelKey: "monitor", Icon: Bell },
  { id: "tools", to: "/tools", labelKey: "tools", Icon: Wrench },
  { id: "network", to: "/activity", labelKey: "networkInfo", Icon: Wifi },
];

/** Inner display / tablet “command center” (~Z Fold unfolded) uses `md` (768px). */
export function AppShell() {
  const { lang, toggleLang, isRtl } = useLanguage();
  const { user, logout } = useAuth();
  const t = (key: keyof typeof UI) => (UI[key] as any)[lang] || UI[key].en;

  const [settings, setSettings] = useState<Record<string, string>>({});

  useEffect(() => {
    fetchSettings();

    const handleUpdate = (e: any) => {
      const { key, value } = e.detail;
      setSettings((prev) => ({ ...prev, [key]: value }));
    };

    window.addEventListener("settings-updated", handleUpdate);
    return () => window.removeEventListener("settings-updated", handleUpdate);
  }, []);

  const fetchSettings = async () => {
    try {
      const res = await fetch("/api/settings");
      if (res.ok) {
        setSettings(await res.json());
      }
    } catch (e) {
      console.error("Failed to fetch settings", e);
    }
  };

  const backdropType = settings["backdrop_type"] || "void";
  const backdropImage = settings["backdrop_image"];

  const backgroundStyle = 
    backdropType === "gray" 
      ? { backgroundColor: "#1a1a1a" }
      : backdropType === "custom" && backdropImage
        ? { 
            backgroundImage: `url(${backdropImage})`,
            backgroundSize: "cover",
            backgroundPosition: "center",
          }
        : { backgroundColor: "#000000" };

  return (
    <div
      dir={isRtl ? "rtl" : "ltr"}
      className="flex h-[100dvh] max-h-[100dvh] flex-col overflow-hidden font-sans text-primary"
      style={backgroundStyle}
    >
      <header className="z-50 shrink-0 border-b border-separator bg-surface/80 backdrop-blur-md pt-[env(safe-area-inset-top)] md:hidden">
        <div className="grid h-16 w-full grid-cols-[1fr_auto_1fr] items-stretch px-[max(0.75rem,env(safe-area-inset-left))] pr-[max(0.75rem,env(safe-area-inset-right))]">
          <div className="min-w-0" aria-hidden />
          <div className="flex min-w-0 items-center justify-center justify-self-center">
            <h1 className="pointer-events-none inline-flex items-center gap-2 text-base font-black leading-none tracking-[0.15em] text-primary sm:text-lg">
              <Radar className="size-4 shrink-0 text-accent sm:size-5" aria-hidden />
              <span className="block truncate">
                {t("appName")}
              </span>
            </h1>
          </div>
          <div className="flex min-w-0 items-center justify-end gap-2">
            <button
              type="button"
              onClick={toggleLang}
              className="rounded-md border border-separator bg-surface-alt px-2 py-1 text-[10px] font-bold uppercase tracking-wide text-secondary transition-colors hover:text-primary"
              title={
                lang === "en" 
                  ? t("switchToArabic") 
                  : lang === "ar" 
                    ? t("switchToGerman") 
                    : t("switchToEnglish")
              }
            >
              {lang === "en" ? "عربي" : lang === "ar" ? "DE" : "EN"}
            </button>
            <NotificationCenterBell />
          </div>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 overflow-hidden">
        <aside className="hidden w-64 shrink-0 flex-col border-r border-separator bg-surface/80 backdrop-blur-md p-5 md:flex">
          <div className="mb-8">
            <div className="flex items-start justify-between gap-2">
              <h1 className="flex min-w-0 items-center gap-2 text-2xl font-semibold text-primary">
                <Radar className="size-6 shrink-0 text-accent" aria-hidden />
                <span className="truncate">{t("appName")}</span>
              </h1>
              <NotificationCenterBell />
            </div>
            <p className="mt-1 text-xs text-secondary">
              {t("spatialPosture")}
            </p>
          </div>

          <nav className="space-y-2 text-sm">
            {sidebarNavigation.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.to === "/"}
                className={({ isActive }) =>
                  cn(
                    "block rounded-md px-3 py-2 text-secondary transition hover:bg-surface-alt",
                    isActive && "bg-accent-muted text-accent",
                  )
                }
              >
                {t(item.key)}
              </NavLink>
            ))}
          </nav>

          {user && (
            <div className="mt-4 rounded-lg border border-accent/20 bg-accent/5 p-3">
              <div className="flex items-center gap-2 text-[10px] font-bold uppercase tracking-wider text-accent mb-2">
                <ShieldCheck className="size-3" />
                {t("adminSession")}
              </div>
              <p className="text-[11px] text-primary truncate font-medium">{user.email}</p>
              <button
                onClick={() => logout()}
                className="mt-3 flex w-full items-center justify-center gap-2 rounded-md border border-separator bg-surface px-2 py-1.5 text-[10px] font-bold uppercase tracking-wide text-secondary transition hover:bg-surface-alt hover:text-primary"
              >
                <LogOut className="size-3" />
                {t("logout")}
              </button>
            </div>
          )}

          <div className="mt-8 rounded-lg border border-separator bg-surface-alt p-4">
            <h2 className="mb-2 text-[10px] font-bold uppercase tracking-widest text-secondary">
              {t("aboutSystem")}
            </h2>
            <div className="space-y-1 text-xs">
              <p className="font-medium text-primary">{t("profileName")}</p>
              <p className="text-secondary leading-relaxed">
                {t("profilePartner")}
              </p>
              <div className="mt-2 flex items-center gap-2 text-[10px] text-accent">
                <span>{t("profileOrigin")}</span>
                <span className="size-1 rounded-full bg-separator" />
                <span>{t("profileLocation")}</span>
              </div>
            </div>
          </div>

          <div className="mt-auto space-y-2">
            <button
              type="button"
              onClick={toggleLang}
              className="w-full rounded-lg border border-separator bg-surface-alt px-3 py-2 text-left text-xs font-semibold text-secondary transition-colors hover:text-primary"
              title={
                lang === "en" 
                  ? t("switchToArabic") 
                  : lang === "ar" 
                    ? t("switchToGerman") 
                    : t("switchToEnglish")
              }
            >
              {lang === "en" 
                ? "🌐 العربية / Arabic" 
                : lang === "ar" 
                  ? "🌐 Deutsch / German" 
                  : "🌐 English / الإنجليزية"}
            </button>
            <div className="rounded-lg border border-separator bg-surface p-4 text-xs text-secondary">
              {t("engineCaption")}
            </div>
          </div>
        </aside>

        <main className="flex min-h-0 flex-1 flex-col overflow-y-auto px-4 py-6 pl-[max(1rem,env(safe-area-inset-left))] pr-[max(1rem,env(safe-area-inset-right))] pb-[calc(6rem+env(safe-area-inset-bottom))] md:p-8 md:pb-8 md:pt-[max(2rem,env(safe-area-inset-top))]">
          <div className="mx-auto flex min-h-0 w-full max-w-[1600px] flex-1 flex-col">
            <Outlet />
          </div>
        </main>
      </div>

      <nav
        aria-label="Primary"
        className="fixed bottom-0 left-0 right-0 z-[100] flex items-center justify-around bg-surface border-t border-separator px-2 pt-2 md:hidden"
        style={{ paddingBottom: "max(8px, env(safe-area-inset-bottom))" }}
      >
        {tabBarItems.map(({ id, to, labelKey, Icon, end }) => (
          <NavLink
            key={id}
            to={to}
            end={end ?? false}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center gap-1 py-1 min-w-[64px] text-[10px] font-medium no-underline transition-colors",
                isActive ? "text-accent" : "text-secondary",
              )
            }
          >
            <Icon className="w-5 h-5" aria-hidden />
            <span>{t(labelKey)}</span>
          </NavLink>
        ))}
      </nav>

      <NotificationCenterPanel />
    </div>
  );
}
