import { useCallback, useEffect, useRef, useState } from "react";
import type { DeviceRow } from "@/hooks/useNetworkScan";
import { useDeviceStore } from "@/stores/deviceStore";

// ── Physics constants ─────────────────────────────────────────────────────────

const REPULSION       = 9500;   // node-node Coulomb repulsion (increased from 4800)
const SPRING_LENGTH   = 280;    // rest length of gateway→device edge (increased from 160)
const SPRING_STRENGTH = 0.012;  // Hooke stiffness (slightly relaxed)
const CENTER_GRAVITY  = 0.006;  // gentle pull toward canvas centre
const DAMPING         = 0.82;   // velocity decay per tick (slightly more momentum)
const GATEWAY_R       = 28;     // gateway node radius (increased from 24)
const NODE_R          = 14;     // device node radius (increased from 11)
const CLICK_MS        = 220;    // max ms for a tap to count as click
const CLICK_PX        = 8;      // max drag distance to still count as click

// ── Zoom/Pan Constants ────────────────────────────────────────────────────────
const MIN_ZOOM = 0.15;
const MAX_ZOOM = 3.0;
const ZOOM_SENSITIVITY = 0.001;

// ── Types ─────────────────────────────────────────────────────────────────────

type PhysicsNode = {
  id: string;
  label: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  pinned: boolean;   // gateway is pinned to canvas centre
  trustScore: number;
  lastSeen: number | null;
  isGateway: boolean;
};

export type NetworkStarMapProps = {
  averageLatencyMs: number | null;
  getTrustScore: (device: DeviceRow) => number;
  onDeviceClick?: (ip: string) => void;
  /** IP of the currently selected device; the matching node gets a selection ring. */
  selectedIp?: string | null;
};

// ── Helpers ───────────────────────────────────────────────────────────────────

function deviceLabel(d: DeviceRow): string {
  if (d.customName?.trim()) return d.customName.trim();
  const rdn = d.hostname?.trim();
  if (rdn) return rdn;
  const mdns = d.mdnsHostname?.trim();
  if (mdns) return mdns;
  const n = d.name?.trim() ?? "";
  if (n && n.toLowerCase() !== "unknown" && n !== d.ip) return n;
  return d.ip;
}

function detectGatewayIp(devices: DeviceRow[]): string | null {
  const dotOne = devices.find((d) => d.ip.endsWith(".1"));
  if (dotOne) return dotOne.ip;
  const sorted = [...devices].sort((a, b) => {
    const ao = parseInt(a.ip.split(".")[3] ?? "255");
    const bo = parseInt(b.ip.split(".")[3] ?? "255");
    return ao - bo;
  });
  return sorted[0]?.ip ?? null;
}

/** Returns [node-colour, glow-colour] for canvas drawing. */
function nodeColors(trustScore: number, lastSeen: number | null, isGateway: boolean): [string, string] {
  if (isGateway) return ["#0A84FF", "#0055CC"];
  const isRecent = lastSeen && (Date.now() - lastSeen < 5 * 60 * 1000);
  if (!isRecent) return ["#636366", "#3A3A3C"];
  if (trustScore >= 75) return ["#30D158", "#1A7A32"];
  if (trustScore >= 50) return ["#0A84FF", "#0055CC"];
  return ["#FF9F0A", "#B36F00"];
}

// ── Main component ────────────────────────────────────────────────────────────

export function NetworkStarMap({ getTrustScore, onDeviceClick, selectedIp }: NetworkStarMapProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef    = useRef<HTMLCanvasElement>(null);
  const nodesRef     = useRef<Map<string, PhysicsNode>>(new Map());
  const rafRef       = useRef<number | null>(null);
  const sizeRef      = useRef({ w: 400, h: 400, dpr: 1 });
  const tickRef      = useRef(0);
  const selectedIpRef = useRef<string | null>(selectedIp ?? null);
  const transformRef = useRef({ x: 0, y: 0, k: 1 });
  
  // Keep getTrustScore in a ref so the subscribe callback always uses the latest version
  // without needing to be in the effect's dependency array.
  const getTrustScoreRef = useRef(getTrustScore);

  // Drag / tap state (refs to avoid re-render churn)
  const dragRef = useRef<{
    id: string | "canvas";
    pointerDownX: number;
    pointerDownY: number;
    downAt: number;
    initialX: number;
    initialY: number;
  } | null>(null);

  // Keep refs in sync with current prop values on every render.
  selectedIpRef.current = selectedIp ?? null;
  getTrustScoreRef.current = getTrustScore;

  // ── Empty-state flag (only thing that needs to trigger a React render) ───────
  const [hasDevices, setHasDevices] = useState(
    () => useDeviceStore.getState().devices.length > 0,
  );

  // ── Canvas resize observer ──────────────────────────────────────────────────

  const [, forceRerender] = useState(0);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect;
      if (!r) return;
      const dpr = Math.min(window.devicePixelRatio ?? 1, 2);
      sizeRef.current = { w: r.width, h: r.height, dpr };
      const canvas = canvasRef.current;
      if (canvas) {
        canvas.width  = Math.round(r.width  * dpr);
        canvas.height = Math.round(r.height * dpr);
        canvas.style.width  = `${r.width}px`;
        canvas.style.height = `${r.height}px`;
      }
      forceRerender((n) => n + 1);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // ── Wheel Zoom ──────────────────────────────────────────────────────────────
  
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = canvas.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;

      const t = transformRef.current;
      const oldK = t.k;
      const nextK = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, t.k * Math.exp(-e.deltaY * ZOOM_SENSITIVITY)));

      if (nextK !== oldK) {
        // Zoom toward mouse pointer
        // (mouseX - t.x) / oldK = (mouseX - nextX) / nextK
        t.x = mouseX - (mouseX - t.x) * (nextK / oldK);
        t.y = mouseY - (mouseY - t.y) * (nextK / oldK);
        t.k = nextK;
      }
    };

    canvas.addEventListener("wheel", handleWheel, { passive: false });
    return () => canvas.removeEventListener("wheel", handleWheel);
  }, []);

  // ── Imperative physics sync: subscribe to store without React re-renders ─────
  // The component re-renders only when hasDevices changes (empty ↔ non-empty).
  // All other device updates (lastSeen, likelyType, etc.) are handled here
  // without touching React state, keeping the canvas always fresh.

  useEffect(() => {
    function syncDevicesToNodes(devs: DeviceRow[]) {
      setHasDevices(devs.length > 0);

      const nodes     = nodesRef.current;
      const { w, h }  = sizeRef.current;
      const cx = w / 2;
      const cy = h / 2;
      const gatewayIp = detectGatewayIp(devs);

      for (const id of nodes.keys()) {
        if (!devs.find((d) => d.ip === id)) nodes.delete(id);
      }

      devs.forEach((d, i) => {
        const isGateway = d.ip === gatewayIp;
        const trust     = getTrustScoreRef.current(d);
        const label     = deviceLabel(d);
        const existing  = nodes.get(d.ip);

        if (existing) {
          existing.label      = label;
          existing.trustScore = trust;
          existing.lastSeen   = d.lastSeen ?? (d.isOnline ? Date.now() : null);
          existing.pinned     = isGateway;
          existing.isGateway  = isGateway;
          if (isGateway) { existing.x = cx; existing.y = cy; }
        } else {
          const angle = (i / Math.max(devs.length, 1)) * Math.PI * 2;
          const r     = 100 + Math.random() * 50;
          nodes.set(d.ip, {
            id: d.ip,
            label,
            x:  isGateway ? cx : cx + Math.cos(angle) * r,
            y:  isGateway ? cy : cy + Math.sin(angle) * r,
            vx: 0,
            vy: 0,
            pinned:     isGateway,
            trustScore: trust,
            lastSeen:   d.lastSeen ?? (d.isOnline ? Date.now() : null),
            isGateway,
          });
        }
      });
    }

    // Initial sync on mount
    syncDevicesToNodes(useDeviceStore.getState().devices);

    // Subscribe to future updates — bypasses React rendering entirely
    return useDeviceStore.subscribe((state) => syncDevicesToNodes(state.devices));
  }, []); // stable: reads getTrustScore via getTrustScoreRef, sizeRef, nodesRef

  // ── Animation + render loop ─────────────────────────────────────────────────

  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    ctxRef.current = canvas.getContext("2d");
    if (!ctxRef.current) return;

    function tick() {
      const ctx = ctxRef.current;
      if (!ctx) return;
      const { w, h, dpr } = sizeRef.current;
      const cx = w / 2;
      const cy = h / 2;
      const tform = transformRef.current;
      tickRef.current++;
      const t = tickRef.current * 0.025;

      const all   = [...nodesRef.current.values()];
      const free  = all.filter((n) => !n.pinned);
      const gw    = all.find((n) => n.pinned) ?? null;

      // ── Physics ──────────────────────────────────────────────────────────
      for (const n of free) {
        if (dragRef.current?.id === n.id) continue; // user is dragging this node

        let fx = 0, fy = 0;

        // Node-node repulsion (O(n²) — fine for <60 nodes)
        for (const other of all) {
          if (other.id === n.id) continue;
          const dx = n.x - other.x;
          const dy = n.y - other.y;
          const d2 = dx * dx + dy * dy + 0.01;
          const d  = Math.sqrt(d2);
          const f  = REPULSION / d2;
          fx += (f * dx) / d;
          fy += (f * dy) / d;
        }

        // Spring to gateway
        if (gw) {
          const dx = gw.x - n.x;
          const dy = gw.y - n.y;
          const d  = Math.sqrt(dx * dx + dy * dy) + 0.001;
          const stretch = d - SPRING_LENGTH;
          fx += SPRING_STRENGTH * stretch * (dx / d);
          fy += SPRING_STRENGTH * stretch * (dy / d);
        }

        // Centre gravity (stops nodes wandering off-canvas)
        fx += (cx - n.x) * CENTER_GRAVITY;
        fy += (cy - n.y) * CENTER_GRAVITY;

        n.vx = (n.vx + fx) * DAMPING;
        n.vy = (n.vy + fy) * DAMPING;
        
        // No hard bounds when zoom/pan is enabled, but keep them reasonably close
        n.x += n.vx;
        n.y += n.vy;
      }

      // ── Draw ─────────────────────────────────────────────────────────────
      ctx.save();
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, w, h);

      // Background
      ctx.fillStyle = "#000000";
      ctx.fillRect(0, 0, w, h);

      // Starfield (static dots seeded by index)
      ctx.save();
      // Apply partial pan to stars for parallax
      ctx.translate(tform.x * 0.1, tform.y * 0.1);
      ctx.fillStyle = "rgba(191,192,196,0.12)";
      for (let i = 0; i < 150; i++) {
        const sx = ((i * 137.508 + 11) % (w * 3)) - w;
        const sy = ((i * 97.311  + 23) % (h * 3)) - h;
        const sr = 0.6 + (i % 3) * 0.3;
        ctx.beginPath();
        ctx.arc(sx, sy, sr, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();

      // Apply zoom/pan transform
      ctx.save();
      ctx.translate(tform.x, tform.y);
      ctx.scale(tform.k, tform.k);

      // Gateway pulse ring
      if (gw) {
        const pulse = 0.35 + 0.12 * Math.sin(t * 1.8);
        ctx.save();
        ctx.globalAlpha = pulse;
        const ringGrad = ctx.createRadialGradient(gw.x, gw.y, GATEWAY_R, gw.x, gw.y, GATEWAY_R * 2.6);
        ringGrad.addColorStop(0, "rgba(10,132,255,0.45)");
        ringGrad.addColorStop(1, "rgba(10,132,255,0)");
        ctx.fillStyle = ringGrad;
        ctx.beginPath();
        ctx.arc(gw.x, gw.y, GATEWAY_R * 2.6, 0, Math.PI * 2);
        ctx.fill();
        ctx.restore();
      }

      // Edges (gateway → each device)
      if (gw) {
        for (const n of free) {
          const isRecent = n.lastSeen && (Date.now() - n.lastSeen < 5 * 60 * 1000);
          ctx.save();
          ctx.globalAlpha = isRecent ? 0.35 : 0.12;
          ctx.strokeStyle = isRecent ? "#0A84FF" : "#636366";
          ctx.lineWidth = 1 / tform.k; // keep line thin regardless of zoom
          ctx.shadowColor  = "#0A84FF";
          ctx.shadowBlur   = 4 / tform.k;
          ctx.setLineDash(isRecent ? [] : [4 / tform.k, 6 / tform.k]);
          ctx.beginPath();
          ctx.moveTo(gw.x, gw.y);
          ctx.lineTo(n.x,  n.y);
          ctx.stroke();
          ctx.restore();
        }
      }

      // Device nodes
      for (const n of all) {
        const r = n.isGateway ? GATEWAY_R : NODE_R;
        const [nodeColor, glowColor] = nodeColors(n.trustScore, n.lastSeen, n.isGateway);
        const isRecent = n.lastSeen && (Date.now() - n.lastSeen < 5 * 60 * 1000);
        const pulse = isRecent && !n.isGateway
          ? 0.55 + 0.20 * Math.sin(t * 1.4 + n.x * 0.01)
          : 1;

        // Glow halo
        ctx.save();
        ctx.globalAlpha = n.isGateway ? 0.5 + 0.1 * Math.sin(t * 1.2) : (isRecent ? 0.38 * pulse : 0.15);
        const halo = ctx.createRadialGradient(n.x, n.y, r * 0.4, n.x, n.y, r * 2.4);
        halo.addColorStop(0, glowColor);
        halo.addColorStop(1, "rgba(0,0,0,0)");
        ctx.fillStyle = halo;
        ctx.beginPath();
        ctx.arc(n.x, n.y, r * 2.4, 0, Math.PI * 2);
        ctx.fill();
        ctx.restore();

        // Node body
        ctx.save();
        ctx.globalAlpha = isRecent ? 1 : 0.5;
        const bodyGrad = ctx.createRadialGradient(n.x - r * 0.3, n.y - r * 0.3, r * 0.1, n.x, n.y, r);
        bodyGrad.addColorStop(0, nodeColor);
        bodyGrad.addColorStop(1, glowColor);
        ctx.fillStyle = bodyGrad;
        ctx.shadowColor = glowColor;
        ctx.shadowBlur  = (n.isGateway ? 18 : 10) / tform.k;
        ctx.beginPath();
        ctx.arc(n.x, n.y, r, 0, Math.PI * 2);
        ctx.fill();
        ctx.restore();

        // Selection ring for the active device
        if (n.id === selectedIpRef.current && !n.isGateway) {
          ctx.save();
          ctx.strokeStyle = "#0A84FF";
          ctx.lineWidth = 2 / tform.k;
          ctx.shadowColor = "#0A84FF";
          ctx.shadowBlur = 10 / tform.k;
          ctx.globalAlpha = 0.85 + 0.15 * Math.sin(t * 2.5);
          ctx.beginPath();
          ctx.arc(n.x, n.y, r + 5, 0, Math.PI * 2);
          ctx.stroke();
          ctx.restore();
        }

        // Gateway inner dot (sun core)
        if (n.isGateway) {
          ctx.save();
          ctx.fillStyle = "#FFFFFF";
          ctx.shadowColor = "rgba(10,132,255,0.6)";
          ctx.shadowBlur  = 6 / tform.k;
          ctx.beginPath();
          ctx.arc(n.x, n.y, r * 0.28, 0, Math.PI * 2);
          ctx.fill();
          ctx.restore();
        }

        // Label
        const labelY = n.y + r + 14 / tform.k;
        ctx.save();
        ctx.globalAlpha = isRecent ? 0.9 : 0.45;
        ctx.font        = `${n.isGateway ? "bold " : ""}${10 / tform.k}px monospace`;
        ctx.textAlign   = "center";
        ctx.textBaseline = "top";
        ctx.shadowColor  = "#000";
        ctx.shadowBlur   = 4 / tform.k;
        ctx.fillStyle    = n.isGateway ? "#409CFF" : (isRecent ? "#8E8E93" : "#636366");
        // Truncate long labels
        const maxChars = n.isGateway ? 18 : 14;
        const text = n.label.length > maxChars ? n.label.slice(0, maxChars - 1) + "…" : n.label;
        ctx.fillText(text, n.x, labelY);
        ctx.restore();
      }

      ctx.restore(); // end of zoom/pan
      ctx.restore(); // end of scale(dpr)
      rafRef.current = requestAnimationFrame(tick);
    }

    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  // ── Pointer interaction (drag + click) ──────────────────────────────────────

  const hitTest = useCallback((clientX: number, clientY: number): PhysicsNode | null => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    const lx   = clientX - rect.left;
    const ly   = clientY - rect.top;
    
    // Reverse transform to get logical coordinates
    const t = transformRef.current;
    const logicalX = (lx - t.x) / t.k;
    const logicalY = (ly - t.y) / t.k;

    let best: PhysicsNode | null = null;
    let bestD = Infinity;

    for (const n of nodesRef.current.values()) {
      const r  = n.isGateway ? GATEWAY_R : NODE_R;
      const dx = n.x - logicalX;
      const dy = n.y - logicalY;
      const d  = Math.sqrt(dx * dx + dy * dy);
      // Adjust hit area for zoom level
      if (d <= (r + 12 / t.k) && d < bestD) { bestD = d; best = n; }
    }
    return best;
  }, []);

  const screenToLogical = useCallback((clientX: number, clientY: number) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    const t = transformRef.current;
    return { 
      x: (clientX - rect.left - t.x) / t.k, 
      y: (clientY - rect.top - t.y) / t.k 
    };
  }, []);

  // Mouse
  const onMouseDown = useCallback((e: React.MouseEvent) => {
    const node = hitTest(e.clientX, e.clientY);
    const t = transformRef.current;
    
    if (node) {
      dragRef.current = {
        id: node.id,
        pointerDownX: e.clientX,
        pointerDownY: e.clientY,
        downAt: Date.now(),
        initialX: node.x,
        initialY: node.y,
      };
    } else {
      // Pan the whole canvas
      dragRef.current = {
        id: "canvas",
        pointerDownX: e.clientX,
        pointerDownY: e.clientY,
        downAt: Date.now(),
        initialX: t.x,
        initialY: t.y,
      };
    }
    e.preventDefault();
  }, [hitTest]);

  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const drag = dragRef.current;
    if (!drag) return;
    
    if (drag.id === "canvas") {
      const dx = e.clientX - drag.pointerDownX;
      const dy = e.clientY - drag.pointerDownY;
      transformRef.current.x = drag.initialX + dx;
      transformRef.current.y = drag.initialY + dy;
    } else {
      const { x, y } = screenToLogical(e.clientX, e.clientY);
      const node = nodesRef.current.get(drag.id);
      if (node && !node.pinned) {
        node.x  = x;
        node.y  = y;
        node.vx = 0;
        node.vy = 0;
      }
    }
  }, [screenToLogical]);

  const onMouseUp = useCallback((e: React.MouseEvent) => {
    const drag = dragRef.current;
    dragRef.current = null;
    if (!drag) return;
    const dt = Date.now() - drag.downAt;
    const dx = Math.abs(e.clientX - drag.pointerDownX);
    const dy = Math.abs(e.clientY - drag.pointerDownY);
    if (dt < CLICK_MS && dx < CLICK_PX && dy < CLICK_PX) {
      const node = nodesRef.current.get(drag.id);
      if (node && !node.isGateway) onDeviceClick?.(node.id);
    }
  }, [onDeviceClick]);

  // Touch
  const onTouchStart = useCallback((e: React.TouchEvent) => {
    const t0 = e.touches[0];
    if (!t0) return;
    const node = hitTest(t0.clientX, t0.clientY);
    const t = transformRef.current;

    if (node) {
      dragRef.current = {
        id: node.id,
        pointerDownX: t0.clientX,
        pointerDownY: t0.clientY,
        downAt: Date.now(),
        initialX: node.x,
        initialY: node.y,
      };
    } else {
      dragRef.current = {
        id: "canvas",
        pointerDownX: t0.clientX,
        pointerDownY: t0.clientY,
        downAt: Date.now(),
        initialX: t.x,
        initialY: t.y,
      };
    }
    e.preventDefault();
  }, [hitTest]);

  const onTouchMove = useCallback((e: React.TouchEvent) => {
    const drag = dragRef.current;
    if (!drag) return;
    const t0 = e.touches[0];
    if (!t0) return;

    if (drag.id === "canvas") {
      const dx = t0.clientX - drag.pointerDownX;
      const dy = t0.clientY - drag.pointerDownY;
      transformRef.current.x = drag.initialX + dx;
      transformRef.current.y = drag.initialY + dy;
    } else {
      const { x, y } = screenToLogical(t0.clientX, t0.clientY);
      const node = nodesRef.current.get(drag.id);
      if (node && !node.pinned) {
        node.x  = x;
        node.y  = y;
        node.vx = 0;
        node.vy = 0;
      }
    }
    e.preventDefault();
  }, [screenToLogical]);

  const onTouchEnd = useCallback((e: React.TouchEvent) => {
    const drag = dragRef.current;
    dragRef.current = null;
    if (!drag) return;
    const dt = Date.now() - drag.downAt;
    const changed = e.changedTouches[0];
    if (!changed) return;
    const dx = Math.abs(changed.clientX - drag.pointerDownX);
    const dy = Math.abs(changed.clientY - drag.pointerDownY);
    if (dt < CLICK_MS && dx < CLICK_PX && dy < CLICK_PX) {
      const node = nodesRef.current.get(drag.id);
      if (node && !node.isGateway) onDeviceClick?.(node.id);
    }
  }, [onDeviceClick]);

  // ── Empty state ──────────────────────────────────────────────────────────────

  if (!hasDevices) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 bg-void text-center">
        <div className="size-16 rounded-full border-2 border-accent/30 bg-accent-muted" />
        <p className="font-mono text-xs uppercase tracking-[0.3em] text-secondary">
          No hosts detected
        </p>
        <p className="text-xs text-secondary">Run a network scan to populate the map.</p>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="absolute inset-0 overflow-hidden bg-void">
      <canvas
        ref={canvasRef}
        className="block touch-none select-none"
        style={{ width: "100%", height: "100%" }}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
        onTouchStart={onTouchStart}
        onTouchMove={onTouchMove}
        onTouchEnd={onTouchEnd}
      />

      {/* Legend */}
      <div className="pointer-events-none absolute bottom-4 left-4 flex flex-col gap-1.5 rounded-xl border border-separator bg-surface/80 px-3 py-2.5">
        <p className="mb-1 text-[9px] font-bold uppercase tracking-[0.25em] text-secondary">Legend</p>
        {[
          { color: "#0A84FF", label: "Gateway / Router" },
          { color: "#30D158", label: "Trust ≥ 75" },
          { color: "#0A84FF", label: "Trust 50–74" },
          { color: "#FF9F0A", label: "Trust < 50" },
          { color: "#636366", label: "Offline" },
        ].map(({ color, label }) => (
          <span key={label} className="flex items-center gap-2">
            <span
              className="size-2.5 shrink-0 rounded-full"
              style={{ background: color }}
            />
            <span className="font-mono text-[10px] text-secondary">{label}</span>
          </span>
        ))}
        <p className="mt-1.5 text-[9px] text-tertiary">Drag nodes · Tap to inspect</p>
      </div>
    </div>
  );
}

export default NetworkStarMap;
