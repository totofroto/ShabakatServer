import * as React from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

type PopoverContextValue = {
  open: boolean;
  setOpen: React.Dispatch<React.SetStateAction<boolean>>;
  triggerRef: React.RefObject<HTMLButtonElement | null>;
  contentRef: React.RefObject<HTMLDivElement | null>;
};

const PopoverContext = React.createContext<PopoverContextValue | null>(null);

function usePopoverContext() {
  const context = React.useContext(PopoverContext);
  if (!context) {
    throw new Error("Popover components must be used within <Popover>.");
  }
  return context;
}

function Popover({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = React.useState(false);
  const triggerRef = React.useRef<HTMLButtonElement>(null);
  const contentRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    if (!open) {
      return;
    }

    const onMouseDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (triggerRef.current?.contains(target) || contentRef.current?.contains(target)) {
        return;
      }
      setOpen(false);
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };

    window.addEventListener("mousedown", onMouseDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <PopoverContext.Provider
      value={{ open, setOpen, triggerRef, contentRef }}
    >
      {children}
    </PopoverContext.Provider>
  );
}

function PopoverTrigger({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  const { open, setOpen, triggerRef } = usePopoverContext();
  return (
    <button
      ref={triggerRef}
      type="button"
      aria-expanded={open}
      onClick={() => setOpen((prev) => !prev)}
      className={cn(className)}
    >
      {children}
    </button>
  );
}

function PopoverContent({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  const { open, triggerRef, contentRef } = usePopoverContext();
  const [style, setStyle] = React.useState<React.CSSProperties>({});

  React.useEffect(() => {
    if (!open || !triggerRef.current) {
      return;
    }
    const rect = triggerRef.current.getBoundingClientRect();
    const top = rect.bottom + window.scrollY + 10;
    const left = rect.left + window.scrollX + rect.width / 2;
    setStyle({ top, left, transform: "translateX(-50%)" });
  }, [open, triggerRef]);

  if (!open) {
    return null;
  }

  return createPortal(
    <div
      ref={contentRef}
      style={style}
      className={cn(
        "absolute z-50 w-80 rounded-lg border border-separator bg-surface p-4 text-left",
        className,
      )}
    >
      {children}
    </div>,
    document.body,
  );
}

export { Popover, PopoverContent, PopoverTrigger };
