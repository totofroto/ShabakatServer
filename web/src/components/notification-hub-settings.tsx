import React, { useState, useEffect } from 'react';
import { Bell, Send, Mail, Link2, CheckCircle2, AlertTriangle, RefreshCw } from 'lucide-react';

interface ProviderConfig {
  id: string;
  name: string;
  enabled: boolean;
  config: Record<string, any>;
}

export const NotificationHubSettings: React.FC = () => {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [saveStatus, setSaveStatus] = useState<Record<string, string>>({});

  // 1. Fetch current live structural channel settings on mount
  useEffect(() => {
    fetch('/api/notifications/config')
      .then((res) => res.json())
      .then((data) => {
        setProviders(data);
        setLoading(false);
      })
      .catch((err) => console.error('[SHABAKAT] Failed to load notification configurations:', err));
  }, []);

  // 2. Dynamic state mutations across nested json field paths
  const handleInputChange = (providerId: string, key: string, value: any) => {
    setProviders((prev) =>
      prev.map((p) =>
        p.id === providerId
          ? { ...p, config: { ...p.config, [key]: value } }
          : p
      )
    );
  };

  const handleToggleProvider = (providerId: string, enabled: boolean) => {
    setProviders((prev) =>
      prev.map((p) => (p.id === providerId ? { ...p, enabled } : p))
    );
  };

  // 3. Persistent atomic update submission to backend engine
  const saveProviderSettings = async (provider: ProviderConfig) => {
    setSaveStatus((prev) => ({ ...prev, [provider.id]: 'saving' }));
    try {
      const res = await fetch('/api/notifications/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          id: provider.id,
          enabled: provider.enabled,
          config_json: provider.config,
        }),
      });

      if (res.ok) {
        setSaveStatus((prev) => ({ ...prev, [provider.id]: 'success' }));
        setTimeout(() => setSaveStatus((prev) => ({ ...prev, [provider.id]: '' })), 2500);
      } else {
        throw new Error(await res.text());
      }
    } catch (err) {
      setSaveStatus((prev) => ({ ...prev, [provider.id]: 'error' }));
    }
  };

  // 4. Instant Connection Diagnostics Verification Trigger
  const testProviderConnection = async (provider: ProviderConfig) => {
    setTestingId(provider.id);
    try {
      // Leverages manual mock routes or explicit test execution payloads
      const res = await fetch('/api/tools/test-notification', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          id: provider.id,
          config: provider.config,
        }),
      });
      
      if (res.ok) {
        alert(`🎉 ${provider.name} Connection verification message dispatched successfully! Check your target feed.`);
      } else {
        alert(`❌ Verification failed: ${await res.text()}`);
      }
    } catch (err) {
      alert(`❌ Diagnostic transmission exception: ${err}`);
    } finally {
      setTestingId(null);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-12 text-secondary font-mono text-sm">
        <RefreshCw className="w-4 h-4 mr-2 animate-spin" /> RUNNING ALARM INFRASTRUCTURE TRACE...
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-4xl mx-auto">
      <div className="flex flex-col space-y-1">
        <h2 className="text-xl font-bold tracking-tight text-primary flex items-center">
          <Bell className="w-5 h-5 mr-2 text-accent" /> Passive Sentry Notification Hub
        </h2>
        <p className="text-sm text-tertiary">
          Configure secure, dynamic alerting thresholds for network perimeters, system breaches, and dynamic out-of-band sentinel logs.
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6">
        {providers.map((provider) => {
          const isSaving = saveStatus[provider.id] === 'saving';
          const isSuccess = saveStatus[provider.id] === 'success';
          const isError = saveStatus[provider.id] === 'error';

          return (
            <div key={provider.id} className="bg-surface border border-separator rounded-xl p-6 shadow-sm space-y-4 transition-all duration-200">
              {/* Header block with interactive slide toggle switch */}
              <div className="flex items-center justify-between border-b border-separator pb-3">
                <div className="flex items-center space-x-3">
                  {provider.id === 'telegram' && <Send className="w-5 h-5 text-blue-400" />}
                  {provider.id === 'smtp' && <Mail className="w-5 h-5 text-green-400" />}
                  {provider.id === 'webhook_ntfy' && <Link2 className="w-5 h-5 text-purple-400" />}
                  <div>
                    <h3 className="font-bold text-sm text-primary">{provider.name}</h3>
                    <span className="text-xs font-mono text-tertiary">ID: {provider.id}</span>
                  </div>
                </div>
                
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={provider.enabled}
                    onChange={(e) => handleToggleProvider(provider.id, e.target.checked)}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-zinc-700 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-accent"></div>
                </label>
              </div>

              {/* Conditional parameters generation matrix */}
              {provider.enabled && (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  {provider.id === 'telegram' && (
                    <>
                      <div className="flex flex-col space-y-1">
                        <label className="text-xs font-semibold text-secondary font-mono">Telegram Bot Token</label>
                        <input
                          type="password"
                          value={provider.config.bot_token || ''}
                          onChange={(e) => handleInputChange(provider.id, 'bot_token', e.target.value)}
                          placeholder="0000000000:AA-ExampleTokenPayloadString"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary font-mono focus:outline-none focus:border-accent"
                        />
                      </div>
                      <div className="flex flex-col space-y-1">
                        <label className="text-xs font-semibold text-secondary font-mono">Chat ID Vector</label>
                        <input
                          type="text"
                          value={provider.config.chat_id || ''}
                          onChange={(e) => handleInputChange(provider.id, 'chat_id', e.target.value)}
                          placeholder="-100000000000"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary font-mono focus:outline-none focus:border-accent"
                        />
                      </div>
                    </>
                  )}

                  {provider.id === 'smtp' && (
                    <>
                      <div className="flex flex-col space-y-1">
                        <label className="text-xs font-semibold text-secondary font-mono">SMTP Server Endpoint</label>
                        <input
                          type="text"
                          value={provider.config.server || ''}
                          onChange={(e) => handleInputChange(provider.id, 'server', e.target.value)}
                          placeholder="smtp.gmail.com"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary font-mono focus:outline-none focus:border-accent"
                        />
                      </div>
                      <div className="flex flex-col space-y-1">
                        <label className="text-xs font-semibold text-secondary font-mono">SMTP Authentication Port</label>
                        <input
                          type="number"
                          value={provider.config.port || 587}
                          onChange={(e) => handleInputChange(provider.id, 'port', parseInt(e.target.value))}
                          placeholder="587"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary font-mono focus:outline-none focus:border-accent"
                        />
                      </div>
                      <div className="flex flex-col space-y-1">
                        <label className="text-xs font-semibold text-secondary font-mono">SMTP Authentication User</label>
                        <input
                          type="text"
                          value={provider.config.user || ''}
                          onChange={(e) => handleInputChange(provider.id, 'user', e.target.value)}
                          placeholder="alerts@example.com"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary focus:outline-none focus:border-accent"
                        />
                      </div>
                      <div className="flex flex-col space-y-1">
                        <label className="text-xs font-semibold text-secondary font-mono">SMTP Access Password</label>
                        <input
                          type="password"
                          value={provider.config.pass || ''}
                          onChange={(e) => handleInputChange(provider.id, 'pass', e.target.value)}
                          placeholder="••••••••••••••••"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary focus:outline-none focus:border-accent"
                        />
                      </div>
                      <div className="flex flex-col space-y-1 md:col-span-2">
                        <label className="text-xs font-semibold text-secondary font-mono">Target Destination Recipient Address ('To')</label>
                        <input
                          type="email"
                          value={provider.config.to || ''}
                          onChange={(e) => handleInputChange(provider.id, 'to', e.target.value)}
                          placeholder="admin@network-estate.internal"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary focus:outline-none focus:border-accent"
                        />
                      </div>
                    </>
                  )}

                  {provider.id === 'webhook_ntfy' && (
                    <>
                      <div className="flex flex-col space-y-1 md:col-span-2">
                        <label className="text-xs font-semibold text-secondary font-mono">ntfy.sh Topic Endpoint URL / Target Route</label>
                        <input
                          type="url"
                          value={provider.config.url || ''}
                          onChange={(e) => handleInputChange(provider.id, 'url', e.target.value)}
                          placeholder="https://ntfy.sh/your_custom_shabakat_channel"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary font-mono focus:outline-none focus:border-accent"
                        />
                      </div>
                      <div className="flex flex-col space-y-1 md:col-span-2">
                        <label className="text-xs font-semibold text-secondary font-mono">HTTP Bearer Authentication Token (Optional)</label>
                        <input
                          type="password"
                          value={provider.config.auth_token || ''}
                          onChange={(e) => handleInputChange(provider.id, 'auth_token', e.target.value)}
                          placeholder="tk_exampelbearerstringparameters"
                          className="bg-surface border border-separator rounded-lg px-3 py-2 text-sm text-primary font-mono focus:outline-none focus:border-accent"
                        />
                      </div>
                    </>
                  )}
                </div>
              )}

              {/* Action layout bar for settings changes and connection validation triggers */}
              <div className="flex items-center justify-end space-x-3 border-t border-separator pt-4">
                <button
                  type="button"
                  onClick={() => testProviderConnection(provider)}
                  disabled={testingId !== null}
                  className="px-3 py-1.5 rounded-lg border border-zinc-600 text-xs font-semibold font-mono text-secondary hover:bg-zinc-800 disabled:opacity-50 transition-all"
                >
                  {testingId === provider.id ? 'TESTING LINK...' : 'TEST CHANNEL'}
                </button>
                <button
                  type="button"
                  onClick={() => saveProviderSettings(provider)}
                  disabled={isSaving}
                  className="px-4 py-1.5 rounded-lg bg-accent text-white text-xs font-bold tracking-wide hover:bg-accent-hover disabled:opacity-50 min-w-[90px] flex items-center justify-center transition-all"
                >
                  {isSaving && <RefreshCw className="w-3 h-3 mr-1 animate-spin" />}
                  {isSuccess && <CheckCircle2 className="w-3 h-3 mr-1 text-green-300" />}
                  {isError && <AlertTriangle className="w-3 h-3 mr-1 text-red-300" />}
                  {isSaving ? 'SAVING...' : isSuccess ? 'SAVED' : isError ? 'RETRY' : 'SAVE CHANGES'}
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
