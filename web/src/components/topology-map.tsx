import { useEffect, useState, useRef, useMemo } from "react";
import { Loader2, Server, Shield, Smartphone, Laptop, Tv, Router, Globe, Cpu, X, Info, Activity, Fingerprint } from "lucide-react";
import * as d3 from "d3";

type TopologyNode = d3.SimulationNodeDatum & {
  id: string;
  label: string;
  ip: string | null;
  isGateway: boolean;
  isServer?: boolean;
  isAdGuard?: boolean;
  isOnline: boolean;
  likelyType: string | null;
  vendor: string | null;
};

type TopologyEdge = d3.SimulationLinkDatum<TopologyNode> & {
  source: string | TopologyNode;
  target: string | TopologyNode;
};

type TopologyData = {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
};

function getIcon(node: TopologyNode) {
  if (node.isGateway) return <Router className="size-full" />;
  if (node.isServer) return <Server className="size-full" />;
  if (node.isAdGuard) return <Shield className="size-full" />;
  
  const type = node.likelyType?.toLowerCase() || "";
  if (type.includes("phone")) return <Smartphone className="size-full" />;
  if (type.includes("laptop") || type.includes("pc")) return <Laptop className="size-full" />;
  if (type.includes("tv")) return <Tv className="size-full" />;
  if (type.includes("iot") || type.includes("esp")) return <Cpu className="size-full" />;
  
  return <Globe className="size-full" />;
}

export function TopologyMap() {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const [data, setData] = useState<TopologyData | null>(null);
  const [loading, setLoading] = useState(true);
  const [dimensions, setDimensions] = useState({ width: 800, height: 400 });
  const [selectedNode, setSelectedNode] = useState<TopologyNode | null>(null);
  
  const [nodes, setNodes] = useState<TopologyNode[]>([]);
  const nodesRef = useRef<TopologyNode[]>([]);
  const [links, setLinks] = useState<TopologyEdge[]>([]);
  const [transform, setTransform] = useState(d3.zoomIdentity);
  const simulationRef = useRef<d3.Simulation<TopologyNode, TopologyEdge> | null>(null);

  // Memoize starfield positions to avoid jitter on every render
  const starfield = useMemo(() => {
    return [...Array(30)].map(() => ({
      width: Math.random() * 2 + 'px',
      height: Math.random() * 2 + 'px',
      top: Math.random() * 100 + '%',
      left: Math.random() * 100 + '%',
    }));
  }, []);

  const fetchTopology = async () => {
    try {
      const res = await fetch("/api/network/topology");
      if (!res.ok) throw new Error("API error");
      const json = await res.json();
      
      setData(prevData => {
        if (!prevData) return json;
        
        // Persist positions from existing nodes to avoid jumpy updates
        const nodeMap = new Map(nodesRef.current.map(n => [n.id, n]));
        json.nodes.forEach((n: TopologyNode) => {
          const prev = nodeMap.get(n.id);
          if (prev) {
            n.x = prev.x;
            n.y = prev.y;
            n.vx = prev.vx;
            n.vy = prev.vy;
          }
        });
        return json;
      });
    } catch (e) {
      console.error("Failed to fetch topology", e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchTopology();
    const interval = setInterval(fetchTopology, 30000); // Polling every 30s is enough
    return () => clearInterval(interval);
  }, []); // Remove nodes dependency to stop infinite loop

  // Handle Resize
  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver((entries) => {
      if (entries[0]) {
        setDimensions({
          width: entries[0].contentRect.width,
          height: entries[0].contentRect.height,
        });
      }
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, []);

  // Initialize Zoom
  useEffect(() => {
    if (!svgRef.current) return;

    const svg = d3.select(svgRef.current);
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.2, 3])
      .on("zoom", (event) => {
        setTransform(event.transform);
      });

    svg.call(zoom);
  }, []);

  // Initialize and update simulation
  useEffect(() => {
    if (!data) return;

    // Stop existing simulation
    if (simulationRef.current) simulationRef.current.stop();

    const newNodes = data.nodes.map(n => ({ ...n }));
    const newLinks = data.edges.map(e => ({
      source: e.source,
      target: e.target
    })) as TopologyEdge[];

    const simulation = d3.forceSimulation<TopologyNode>(newNodes)
      .alphaMin(0.05) // Stop simulation earlier
      .velocityDecay(0.7) // Add friction
      .force("link", d3.forceLink<TopologyNode, TopologyEdge>(newLinks).id(d => d.id).distance(140))
      .force("charge", d3.forceManyBody().strength(-150))
      .force("center", d3.forceCenter(dimensions.width / 2, dimensions.height / 2))
      .force("collision", d3.forceCollide().radius(65));

    simulation.on("tick", () => {
      nodesRef.current = [...newNodes];
      setNodes([...newNodes]);
      setLinks([...newLinks]);
    });

    simulationRef.current = simulation;

    return () => {
      simulation.stop();
    };
  }, [data, dimensions.width, dimensions.height]);

  // Drag behavior binding
  useEffect(() => {
    if (!svgRef.current || nodes.length === 0) return;
    
    const svg = d3.select(svgRef.current);
    const simulation = simulationRef.current;

    nodes.forEach(node => {
      const nodeElement = svg.select(`#node-${node.id.replace(/:/g, '-')}`);
      
      const dragBehavior = d3.drag<any, TopologyNode>()
        .on("start", (event) => {
          if (!event.active && simulation) simulation.alphaTarget(0.3).restart();
          node.fx = node.x;
          node.fy = node.y;
        })
        .on("drag", (event) => {
          node.fx = event.x;
          node.fy = event.y;
        })
        .on("end", (event) => {
          if (!event.active && simulation) simulation.alphaTarget(0);
          node.fx = null;
          node.fy = null;
        });

      nodeElement.call(dragBehavior as any);
    });
  }, [nodes]);

  if (loading && !data) {
    return (
      <div className="flex h-full items-center justify-center bg-void">
        <Loader2 className="size-6 animate-spin text-accent" />
      </div>
    );
  }

  if (!data || data.nodes.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-void text-[10px] font-bold uppercase tracking-widest text-tertiary">
        No active nodes detected
      </div>
    );
  }

  return (
    <div 
      ref={containerRef} 
      className="relative size-full overflow-hidden bg-void"
      onClick={() => setSelectedNode(null)}
    >
      {/* Starfield background */}
      <div className="absolute inset-0 opacity-10 pointer-events-none">
        {starfield.map((star, i) => (
          <div
            key={i}
            className="absolute rounded-full bg-white"
            style={star}
          />
        ))}
      </div>

      <svg 
        ref={svgRef} 
        className="relative z-10 size-full select-none cursor-grab active:cursor-grabbing"
      >
        <defs>
          <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feComposite in="SourceGraphic" in2="blur" operator="over" />
          </filter>
        </defs>

        <g transform={transform.toString()}>
          {/* Edges */}
          {links.map((link, i) => {
            const source = typeof link.source === 'object' ? link.source : null;
            const target = typeof link.target === 'object' ? link.target : null;
            if (!source || !target) return null;

            const isOnline = target.isOnline;

            return (
              <line
                key={`edge-${i}`}
                x1={source.x}
                y1={source.y}
                x2={target.x}
                y2={target.y}
                stroke={isOnline ? "var(--accent)" : "var(--bg-separator)"}
                strokeWidth={isOnline ? "1.5" : "1"}
                strokeDasharray={isOnline ? "none" : "4 4"}
                className="opacity-20"
              />
            );
          })}

          {/* Nodes */}
          {nodes.map((node) => {
            if (node.x === undefined || node.y === undefined) return null;

            const isImportant = node.isGateway || node.isServer || node.isAdGuard;
            const isOnline = node.isOnline;
            const isSelected = selectedNode?.id === node.id;
            const color = node.isGateway 
              ? "#0A84FF" 
              : node.isServer 
                ? "#30D158" 
                : node.isAdGuard 
                  ? "#FF9F0A" 
                  : isOnline ? "#FFFFFF" : "#636366";

            return (
              <g 
                key={node.id} 
                id={`node-${node.id.replace(/:/g, '-')}`}
                className="cursor-pointer group"
                onClick={(e) => {
                   e.stopPropagation();
                   setSelectedNode(node);
                   console.log(`Device Selected: ${node.label} [${node.ip || 'No IP'}] MAC: ${node.id}`);
                }}
              >
                {/* Glow for important nodes */}
                {(isImportant || node.isGateway || isSelected) && isOnline && (
                  <circle
                    cx={node.x}
                    cy={node.y}
                    r={node.isGateway ? 24 : 18}
                    fill={color}
                    className={`${isSelected ? "opacity-40" : "opacity-20"} group-hover:opacity-40 transition-opacity`}
                    filter="url(#glow)"
                  />
                )}

                {/* Node Circle */}
                <circle
                  cx={node.x}
                  cy={node.y}
                  r={node.isGateway ? 16 : 12}
                  fill="var(--bg-surface)"
                  stroke={isSelected ? "#FFFFFF" : color}
                  strokeWidth={isSelected ? "3" : "2"}
                  className={`${isOnline ? "" : "opacity-50"} group-hover:stroke-white transition-all`}
                />

                {/* Icon */}
                <foreignObject
                  x={node.x - (node.isGateway ? 8 : 6)}
                  y={node.y - (node.isGateway ? 8 : 6)}
                  width={node.isGateway ? 16 : 12}
                  height={node.isGateway ? 16 : 12}
                  className={`${isOnline ? "" : "opacity-40"} pointer-events-none`}
                  style={{ color: isSelected ? "#FFFFFF" : color }}
                >
                  {getIcon(node)}
                </foreignObject>

                {/* Label */}
                <text
                  x={node.x}
                  y={node.y + (node.isGateway ? 28 : 24)}
                  textAnchor="middle"
                  className={`${isSelected ? "fill-white scale-110" : "fill-secondary"} font-mono text-[9px] font-bold uppercase tracking-tighter pointer-events-none transition-all`}
                >
                  {node.label.length > 15 ? node.label.slice(0, 13) + ".." : node.label}
                </text>
                
                {/* IP Address */}
                {isOnline && (
                  <text
                    x={node.x}
                    y={node.y + (node.isGateway ? 38 : 34)}
                    textAnchor="middle"
                    className="fill-tertiary font-mono text-[7px] pointer-events-none"
                  >
                    {node.ip}
                  </text>
                )}
              </g>
            );
          })}
        </g>
      </svg>

      {/* Device Details Panel */}
      {selectedNode && (
        <div 
          className="absolute left-6 top-1/2 z-20 w-72 -translate-y-1/2 animate-in fade-in slide-in-from-left-4 duration-300"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="relative overflow-hidden rounded-2xl border border-white/10 bg-[#0a0a0b]/80 p-5 shadow-2xl backdrop-blur-xl">
            {/* Header */}
            <div className="mb-4 flex items-start justify-between">
              <div>
                <h3 className="text-lg font-black uppercase tracking-tight text-white leading-tight">
                  {selectedNode.label}
                </h3>
                <p className="text-[10px] font-bold uppercase tracking-widest text-accent mt-1">
                  {selectedNode.isGateway ? "Network Gateway" : selectedNode.likelyType || "Unknown Device"}
                </p>
              </div>
              <button 
                onClick={() => setSelectedNode(null)}
                className="rounded-full p-1 text-tertiary hover:bg-white/10 hover:text-white transition-colors"
              >
                <X className="size-4" />
              </button>
            </div>

            {/* Info Grid */}
            <div className="space-y-4">
              <div className="flex items-center gap-3">
                <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-white/5 text-accent">
                  <Activity className="size-4" />
                </div>
                <div>
                  <p className="text-[8px] font-bold uppercase tracking-widest text-tertiary">Network Address</p>
                  <p className="font-mono text-xs text-secondary">{selectedNode.ip || "NO IP ASSIGNED"}</p>
                </div>
              </div>

              <div className="flex items-center gap-3">
                <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-white/5 text-accent">
                  <Fingerprint className="size-4" />
                </div>
                <div>
                  <p className="text-[8px] font-bold uppercase tracking-widest text-tertiary">Hardware ID (MAC)</p>
                  <p className="font-mono text-xs text-secondary uppercase">{selectedNode.id}</p>
                </div>
              </div>

              <div className="flex items-center gap-3">
                <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-white/5 text-accent">
                  <Info className="size-4" />
                </div>
                <div>
                  <p className="text-[8px] font-bold uppercase tracking-widest text-tertiary">Manufacturer</p>
                  <p className="text-xs text-secondary truncate w-40">{selectedNode.vendor || "UNIDENTIFIED"}</p>
                </div>
              </div>
            </div>

            {/* Status Indicator */}
            <div className="mt-6 flex items-center justify-between rounded-xl bg-white/5 p-3">
              <span className="text-[9px] font-black uppercase tracking-widest text-tertiary">Status</span>
              <div className="flex items-center gap-2">
                <div className={`size-2 rounded-full ${selectedNode.isOnline ? "bg-[#30D158] shadow-[0_0_8px_#30D158]" : "bg-red-500"}`} />
                <span className="text-[10px] font-bold uppercase tracking-widest text-white">
                  {selectedNode.isOnline ? "CONNECTED" : "OFFLINE"}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Floating HUD Labels */}
      <div className="pointer-events-none absolute top-6 right-6 flex flex-col gap-1 text-right">
        <p className="text-[10px] font-black uppercase tracking-[0.2em] text-accent">Real-Time Topology</p>
        <p className="text-[8px] font-bold uppercase tracking-widest text-tertiary">
          Nodes: {data.nodes.length} · Active: {data.nodes.filter(n => n.isOnline).length}
        </p>
      </div>

      <div className="pointer-events-none absolute top-6 left-6">
        <p className="text-[8px] font-bold uppercase tracking-widest text-tertiary opacity-40">
          [DRAG TO MOVE · SCROLL TO ZOOM · PAN TO EXPLORE]
        </p>
      </div>

      {/* Legend */}
      <div className="pointer-events-none absolute bottom-6 left-6 flex gap-4 rounded-lg border border-separator/20 bg-surface/30 p-2 backdrop-blur-md">
        <div className="flex items-center gap-1.5">
          <div className="size-1.5 rounded-full bg-[#0A84FF] shadow-[0_0_8px_#0A84FF]" />
          <span className="text-[8px] font-bold uppercase tracking-widest text-secondary">Gateway</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="size-1.5 rounded-full bg-[#30D158] shadow-[0_0_8px_#30D158]" />
          <span className="text-[8px] font-bold uppercase tracking-widest text-secondary">NAS Node</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="size-1.5 rounded-full bg-[#FF9F0A] shadow-[0_0_8px_#FF9F0A]" />
          <span className="text-[8px] font-bold uppercase tracking-widest text-secondary">Security Node</span>
        </div>
      </div>
    </div>
  );
}
