import { useEffect, useRef, useState } from "react";
import { transport } from "@/lib/transport";
import { Terminal, Cpu } from "lucide-react";

interface LogLine {
  id: number;
  text: string;
  level: "info" | "warn" | "error" | "system";
}

export function DevConsole() {
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [command, setCommand] = useState("");
  const [terminalOutput, setTerminalOutput] = useState<string>("");
  const [isExecuting, setIsExecuting] = useState(false);
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // We use a direct fetch or EventSource here. 
    // Since it's a relative path, we can just use the path.
    const eventSource = new EventSource("/api/debug/logs/stream");

    eventSource.onmessage = (event) => {
      const text = event.data;
      let level: LogLine["level"] = "info";
      if (text.includes("[WARN]")) level = "warn";
      else if (text.includes("[ERROR]")) level = "error";
      else if (text.includes("[SYSTEM]")) level = "system";

      setLogs((prev) => [
        ...prev.slice(-499),
        { id: Date.now() + Math.random(), text, level },
      ]);
    };

    eventSource.onerror = (err) => {
      console.error("[FLIGHT_RECORDER] Log stream error:", err);
    };

    return () => {
      eventSource.close();
    };
  }, []);

  useEffect(() => {
    if (logEndRef.current) {
      logEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [logs]);

  async function handleRunCommand(e: React.FormEvent) {
    e.preventDefault();
    const cmd = command.trim();
    if (!cmd || isExecuting) return;

    setIsExecuting(true);
    setTerminalOutput((prev) => prev + `\n$ ${cmd}\n`);
    setCommand("");

    try {
      const res = await transport.fetch("/api/debug/terminal/run", {
        method: "POST",
        body: JSON.stringify({ command: cmd }),
      });
      
      if (!res.ok) {
        const errorData = await res.json().catch(() => ({ output: `HTTP ${res.status}: ${res.statusText}` }));
        setTerminalOutput((prev) => prev + (errorData.output || `Error: ${res.statusText}`) + "\n");
      } else {
        const data = await res.json();
        setTerminalOutput((prev) => prev + data.output + "\n");
      }
    } catch (err) {
      setTerminalOutput((prev) => prev + `Error: ${err}\n`);
    } finally {
      setIsExecuting(false);
    }
  }

  return (
    <div className="flex flex-col gap-4 h-[calc(100vh-250px)] min-h-[500px]">
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 flex-1 overflow-hidden">
        {/* Left Pane: Live Logs */}
        <div className="flex flex-col bg-slate-950 border border-slate-800/50 rounded-xl overflow-hidden shadow-2xl">
          <div className="flex items-center justify-between px-4 py-2.5 bg-slate-900/50 border-b border-slate-800/50">
            <div className="flex items-center gap-2">
              <Cpu className="w-4 h-4 text-emerald-500" />
              <span className="text-[10px] font-black uppercase tracking-widest text-slate-400">Live System Logs</span>
            </div>
            <div className="flex items-center gap-1.5">
               <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
               <span className="text-[9px] text-emerald-500 font-black tracking-tighter">STREAMING</span>
            </div>
          </div>
          <div className="flex-1 p-4 font-mono text-[11px] overflow-y-auto space-y-1 bg-black/20 scrollbar-thin scrollbar-thumb-slate-800">
            {logs.length === 0 && (
              <div className="text-slate-600 italic">Waiting for logs...</div>
            )}
            {logs.map((log) => (
              <div 
                key={log.id} 
                className={`break-all leading-relaxed ${
                  log.level === 'error' ? 'text-rose-400' : 
                  log.level === 'warn' ? 'text-amber-400' : 
                  log.level === 'system' ? 'text-sky-400' : 
                  'text-slate-300'
                }`}
              >
                {log.text}
              </div>
            ))}
            <div ref={logEndRef} />
          </div>
        </div>

        {/* Right Pane: Interactive Terminal */}
        <div className="flex flex-col bg-slate-950 border border-slate-800/50 rounded-xl overflow-hidden shadow-2xl">
          <div className="flex items-center gap-2 px-4 py-2.5 bg-slate-900/50 border-b border-slate-800/50">
            <Terminal className="w-4 h-4 text-sky-500" />
            <span className="text-[10px] font-black uppercase tracking-widest text-slate-400">Secure Web-Shell</span>
          </div>
          <div className="flex-1 p-4 font-mono text-[11px] overflow-y-auto whitespace-pre-wrap bg-black/20 text-emerald-500/90 scrollbar-thin scrollbar-thumb-slate-800">
            {terminalOutput || "Authorized access only. Whitelisted diagnostic tools: ls, cat, arp, ping, df, ps."}
          </div>
          <form onSubmit={(e) => void handleRunCommand(e)} className="p-3 bg-slate-900/30 border-t border-slate-800/50">
            <div className="flex items-center gap-2 px-3 py-1.5 bg-black/40 rounded-lg border border-slate-700/50 focus-within:border-sky-500/40 focus-within:ring-1 focus-within:ring-sky-500/20 transition-all">
              <span className="text-sky-500 font-bold text-sm">$</span>
              <input
                type="text"
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                placeholder="Type command..."
                className="flex-1 bg-transparent border-none outline-none text-slate-200 text-sm font-mono placeholder:text-slate-700"
                disabled={isExecuting}
                autoComplete="off"
                spellCheck="false"
              />
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}
