import { useEffect, useState, useRef } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useLanguage } from "@/context/LanguageContext";
import { useAuth } from "@/context/AuthContext";
import { DnsProvider } from "@/types";
import { Plus, Trash2, ShieldCheck, Power, Palette, Upload, Check, LogIn, ShieldAlert } from "lucide-react";

const UI: Record<string, { en: string; ar: string }> = {
  title: { en: "Settings", ar: "الإعدادات" },
  desc: {
    en: "Configure scanner behavior and notifications (coming soon).",
    ar: "قم بضبط سلوك الفحص والإشعارات (قريبًا).",
  },
  scannerConfig: { en: "Scanner Configuration", ar: "إعدادات الفاحص" },
  scannerConfigDesc: {
    en: "Placeholder options for scan interval, subnet ranges, and alert policies.",
    ar: "خيارات تجريبية لفترة الفحص ونطاقات الشبكة وسياسات التنبيه.",
  },
  scanInterval: { en: "Scan interval: 15 min (placeholder)", ar: "فترة الفحص: 15 دقيقة (تجريبي)" },
  subnet: { en: "Monitored subnet: 192.168.1.0/24", ar: "الشبكة الفرعية المراقبة: 192.168.1.0/24" },
  sensitivity: { en: "Alert sensitivity: Medium", ar: "حساسية التنبيه: متوسطة" },
  channel: { en: "Notification channel: Desktop", ar: "قناة الإشعارات: سطح المكتب" },
  personalizationTitle: { en: "Personalization", ar: "التخصيص" },
  personalizationDesc: { en: "Configure your backdrop and visual experience.", ar: "قم بضبط الخلفية والخبرة البصرية الخاصة بك." },
  backdropVoid: { en: "True Black Void", ar: "الفراغ الأسود الحقيقي" },
  backdropGray: { en: "Tactical Dark Gray", ar: "الرمادي التكتيكي الغامق" },
  backdropCustom: { en: "Custom Blueprint", ar: "مخطط مخصص" },
  uploadWallpaper: { en: "Upload Custom Blueprint Wallpaper", ar: "تحميل خلفية مخصصة" },
  dnsTitle: { en: "DNS Monitoring", ar: "مراقبة DNS" },
  dnsDesc: { en: "Manage AdGuard Home or other DNS providers for device security stats.", ar: "إدارة AdGuard Home أو مزودي DNS الآخرين لإحصائيات أمان الأجهزة." },
  addProvider: { en: "Save Provider", ar: "حفظ المزود" },
  providerName: { en: "Name (e.g. Home AdGuard)", ar: "الاسم (مثلاً AdGuard المنزلي)" },
  providerIp: { en: "IP Address", ar: "عنوان IP" },
  providerPort: { en: "Port", ar: "المنفذ" },
  providerUser: { en: "Username (optional)", ar: "اسم المستخدم (اختياري)" },
  providerPass: { en: "Password (optional)", ar: "كلمة المرور (اختياري)" },
  active: { en: "Active", ar: "نشط" },
  disabled: { en: "Disabled", ar: "معطل" },
  accessTitle: { en: "System Access", ar: "الوصول إلى النظام" },
  accessDesc: { en: "Manage your administrative session and authentication.", ar: "إدارة جلسة العمل الإدارية والتحقق من الهوية." },
  loginWithGoogle: { en: "Sign in with Google", ar: "تسجيل الدخول باستخدام جوجل" },
  signOut: { en: "Sign Out", ar: "تسجيل الخروج" },
};

export function SettingsPage() {
  const { lang } = useLanguage();
  const { user } = useAuth();
  const t = (key: string) => (UI[key] as any)?.[lang] || UI[key]?.en || key;

  const [providers, setProviders] = useState<DnsProvider[]>([]);
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [newProvider, setNewProvider] = useState({
    name: "",
    ip: "",
    port: 80,
    username: "",
    password: "",
  });

  useEffect(() => {
    fetchProviders();
    fetchSettings();
  }, []);

  const fetchProviders = async () => {
    try {
      const res = await fetch("/api/dns/providers");
      if (res.ok) {
        setProviders(await res.json());
      }
    } catch (e) {
      console.error("Failed to fetch DNS providers", e);
    }
  };

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

  const updateSetting = async (key: string, value: string) => {
    try {
      const res = await fetch("/api/settings", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key, value }),
      });
      if (res.ok) {
        setSettings((prev) => ({ ...prev, [key]: value }));
        window.dispatchEvent(new CustomEvent("settings-updated", { detail: { key, value } }));
      }
    } catch (e) {
      console.error("Failed to update setting", e);
    }
  };

  const handleWallpaperUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    setUploading(true);
    const formData = new FormData();
    formData.append("file", file);

    try {
      const res = await fetch("/api/assets/upload", {
        method: "POST",
        body: formData,
      });
      if (res.ok) {
        const { url } = await res.json();
        await updateSetting("backdrop_image", url);
        await updateSetting("backdrop_type", "custom");
      }
    } catch (e) {
      console.error("Failed to upload wallpaper", e);
    } finally {
      setUploading(false);
    }
  };

  const handleAddProvider = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newProvider.name || !newProvider.ip) return;

    try {
      const res = await fetch("/api/dns/providers", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(newProvider),
      });
      if (res.ok) {
        setNewProvider({ name: "", ip: "", port: 80, username: "", password: "" });
        fetchProviders();
      }
    } catch (e) {
      console.error("Failed to add DNS provider", e);
    }
  };

  const toggleProvider = async (id: string, currentStatus: boolean) => {
    try {
      const res = await fetch(`/api/dns/providers/${id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ isEnabled: !currentStatus }),
      });
      if (res.ok) {
        fetchProviders();
      }
    } catch (e) {
      console.error("Failed to toggle DNS provider", e);
    }
  };

  const deleteProvider = async (id: string) => {
    try {
      const res = await fetch(`/api/dns/providers/${id}`, {
        method: "DELETE",
      });
      if (res.ok) {
        fetchProviders();
      }
    } catch (e) {
      console.error("Failed to delete DNS provider", e);
    }
  };

  const handleHardcodedLogin = () => {
    window.location.href = "http://192.168.254.18.nip.io:7779/api/auth/google/login";
  };

  const handleLogout = async () => {
    try {
      await fetch("/api/auth/logout", { method: "POST" });
      window.location.reload();
    } catch (e) {
      console.error("Logout failed", e);
    }
  };

  const activeBackdrop = settings["backdrop_type"] || "void";

  return (
    <div className="space-y-6">
      <header>
        <h2 className="text-3xl font-semibold text-primary">{t("title")}</h2>
        <p className="mt-1 text-sm text-secondary">
          {t("desc")}
        </p>
      </header>

      {/* System Access Block */}
      <Card className="border-accent/20 bg-accent/5">
        <CardHeader>
          <div className="flex items-center gap-2">
            <ShieldAlert className="h-5 w-5 text-accent" />
            <CardTitle className="text-primary">{t("accessTitle")}</CardTitle>
          </div>
          <CardDescription className="text-secondary">
            {t("accessDesc")}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div className="text-sm">
              <p className="font-medium text-primary">
                {user ? `Logged in as ${user.email}` : "Not signed in"}
              </p>
              <p className="text-xs text-secondary">Bypass internal state with hardcoded redirect.</p>
            </div>
            <div className="flex gap-2">
              {user && (
                <Button
                  onClick={handleLogout}
                  variant="outline"
                  className="border-destructive/50 text-destructive hover:bg-destructive/10"
                >
                  <Power className="w-4 h-4 mr-2" />
                  {t("signOut")}
                </Button>
              )}
              <Button
                onClick={handleHardcodedLogin}
                className="bg-white text-black hover:bg-zinc-200 font-semibold"
              >
                <LogIn className="w-4 h-4 mr-2" />
                {t("loginWithGoogle")}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Personalization Block */}
      <Card className="border-primary/20 bg-surface/50">
        <CardHeader>
          <div className="flex items-center gap-2">
            <Palette className="h-5 w-5 text-primary" />
            <CardTitle className="text-primary">{t("personalizationTitle")}</CardTitle>
          </div>
          <CardDescription className="text-secondary">
            {t("personalizationDesc")}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="grid gap-4 sm:grid-cols-3">
            <button
              onClick={() => updateSetting("backdrop_type", "void")}
              className={`relative flex flex-col items-center gap-2 rounded-lg border p-4 transition-all ${
                activeBackdrop === "void" ? "border-primary bg-primary/10" : "border-separator bg-surface-alt hover:border-secondary"
              }`}
            >
              <div className="h-12 w-full rounded bg-[#000000]" />
              <span className="text-xs font-medium">{t("backdropVoid")}</span>
              {activeBackdrop === "void" && <Check className="absolute top-2 right-2 h-3 w-3 text-primary" />}
            </button>

            <button
              onClick={() => updateSetting("backdrop_type", "gray")}
              className={`relative flex flex-col items-center gap-2 rounded-lg border p-4 transition-all ${
                activeBackdrop === "gray" ? "border-primary bg-primary/10" : "border-separator bg-surface-alt hover:border-secondary"
              }`}
            >
              <div className="h-12 w-full rounded bg-[#1a1a1a]" />
              <span className="text-xs font-medium">{t("backdropGray")}</span>
              {activeBackdrop === "gray" && <Check className="absolute top-2 right-2 h-3 w-3 text-primary" />}
            </button>

            <div className="flex flex-col gap-2">
              <button
                onClick={() => settings["backdrop_image"] && updateSetting("backdrop_type", "custom")}
                disabled={!settings["backdrop_image"]}
                className={`relative flex flex-col items-center gap-2 rounded-lg border p-4 transition-all flex-1 ${
                  activeBackdrop === "custom" ? "border-primary bg-primary/10" : "border-separator bg-surface-alt hover:border-secondary disabled:opacity-50"
                }`}
              >
                {settings["backdrop_image"] ? (
                  <div 
                    className="h-12 w-full rounded bg-cover bg-center" 
                    style={{ backgroundImage: `url(${settings["backdrop_image"]})` }}
                  />
                ) : (
                  <div className="flex h-12 w-full items-center justify-center rounded bg-surface-alt border border-dashed border-secondary/30">
                    <Palette className="h-5 w-5 text-secondary/30" />
                  </div>
                )}
                <span className="text-xs font-medium">{t("backdropCustom")}</span>
                {activeBackdrop === "custom" && <Check className="absolute top-2 right-2 h-3 w-3 text-primary" />}
              </button>
              
              <input
                type="file"
                ref={fileInputRef}
                onChange={handleWallpaperUpload}
                accept="image/png,image/jpeg"
                className="hidden"
              />
              <Button
                variant="outline"
                size="sm"
                className="w-full gap-2 text-[10px] h-8"
                onClick={() => fileInputRef.current?.click()}
                disabled={uploading}
              >
                <Upload className="h-3 w-3" />
                {t("uploadWallpaper")}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* DNS Provider Management Block */}
      <Card className="border-primary/20 bg-surface/50">
        <CardHeader>
          <div className="flex items-center gap-2">
            <ShieldCheck className="h-5 w-5 text-primary" />
            <CardTitle className="text-primary">{t("dnsTitle")}</CardTitle>
          </div>
          <CardDescription className="text-secondary">
            {t("dnsDesc")}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Active Providers List */}
          <div className="grid gap-3">
            {providers.map((p) => (
              <div
                key={p.id}
                className={`flex items-center justify-between rounded-lg border p-4 transition-colors ${
                  p.isEnabled 
                    ? "border-primary/30 bg-primary/5" 
                    : "border-separator bg-surface-alt opacity-60"
                }`}
              >
                <div className="flex items-center gap-3">
                  <div className={`rounded-full p-2 ${p.isEnabled ? "bg-primary/20" : "bg-secondary/20"}`}>
                    <Power className={`h-4 w-4 ${p.isEnabled ? "text-primary" : "text-secondary"}`} />
                  </div>
                  <div>
                    <h4 className="font-medium text-primary">{p.name}</h4>
                    <p className="text-xs text-secondary">{p.ip}:{p.port}</p>
                  </div>
                </div>
                
                <div className="flex items-center gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => toggleProvider(p.id, p.isEnabled)}
                    className={p.isEnabled ? "text-primary" : "text-secondary"}
                  >
                    {p.isEnabled ? t("active") : t("disabled")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => deleteProvider(p.id)}
                    className="text-destructive hover:bg-destructive/10"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            ))}
          </div>

          {/* Inline Add Form */}
          <form onSubmit={handleAddProvider} className="mt-4 grid gap-4 rounded-md border border-separator bg-surface-alt/30 p-4 md:grid-cols-3">
            <div className="space-y-1">
              <label className="text-xs font-medium text-secondary">{t("providerName")}</label>
              <input
                type="text"
                value={newProvider.name}
                onChange={(e) => setNewProvider({ ...newProvider, name: e.target.value })}
                className="w-full rounded-md border border-separator bg-surface p-2 text-sm text-primary focus:border-primary focus:outline-none"
                required
              />
            </div>
            <div className="space-y-1">
              <label className="text-xs font-medium text-secondary">{t("providerIp")}</label>
              <input
                type="text"
                value={newProvider.ip}
                onChange={(e) => setNewProvider({ ...newProvider, ip: e.target.value })}
                className="w-full rounded-md border border-separator bg-surface p-2 text-sm text-primary focus:border-primary focus:outline-none"
                placeholder="192.168.1.100"
                required
              />
            </div>
            <div className="space-y-1">
              <label className="text-xs font-medium text-secondary">{t("providerPort")}</label>
              <input
                type="number"
                value={newProvider.port}
                onChange={(e) => setNewProvider({ ...newProvider, port: parseInt(e.target.value) || 80 })}
                className="w-full rounded-md border border-separator bg-surface p-2 text-sm text-primary focus:border-primary focus:outline-none"
              />
            </div>
            <div className="space-y-1">
              <label className="text-xs font-medium text-secondary">{t("providerUser")}</label>
              <input
                type="text"
                value={newProvider.username}
                onChange={(e) => setNewProvider({ ...newProvider, username: e.target.value })}
                className="w-full rounded-md border border-separator bg-surface p-2 text-sm text-primary focus:border-primary focus:outline-none"
              />
            </div>
            <div className="space-y-1">
              <label className="text-xs font-medium text-secondary">{t("providerPass")}</label>
              <input
                type="password"
                value={newProvider.password}
                onChange={(e) => setNewProvider({ ...newProvider, password: e.target.value })}
                className="w-full rounded-md border border-separator bg-surface p-2 text-sm text-primary focus:border-primary focus:outline-none"
              />
            </div>
            <div className="flex items-end">
              <Button type="submit" className="w-full gap-2">
                <Plus className="h-4 w-4" />
                {t("addProvider")}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-primary">{t("scannerConfig")}</CardTitle>
          <CardDescription className="text-secondary">
            {t("scannerConfigDesc")}
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm text-secondary md:grid-cols-2">
          <div className="rounded-md border border-separator bg-surface-alt p-3">
            {t("scanInterval")}
          </div>
          <div className="rounded-md border border-separator bg-surface-alt p-3">
            {t("subnet")}
          </div>
          <div className="rounded-md border border-separator bg-surface-alt p-3">
            {t("sensitivity")}
          </div>
          <div className="rounded-md border border-separator bg-surface-alt p-3">
            {t("channel")}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
