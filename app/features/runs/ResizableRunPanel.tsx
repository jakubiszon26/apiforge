import { useEffect, useRef, useState, type ReactNode } from 'react';

/** Capture the pointer so resizing continues outside the narrow divider. */
export function ResizableRunPanel({ children, hidden }: { children: ReactNode; hidden: boolean }) {
  const [height, setHeight] = useState(320);
  const [maximum, setMaximum] = useState(520);
  const [resizing, setResizing] = useState(false);
  const panel = useRef<HTMLDivElement>(null);
  const drag = useRef<{ y: number; height: number } | null>(null);
  const clamp = (value: number) => Math.max(160, Math.min(value, maximum));
  useEffect(() => {
    const resize = () => {
      const max = Math.max(160, window.innerHeight - 480);
      setMaximum(max);
      setHeight((current) => Math.max(160, Math.min(current, max)));
    };
    resize();
    window.addEventListener('resize', resize);
    return () => window.removeEventListener('resize', resize);
  }, []);

  return (
    <div
      ref={panel}
      className={`af-resizable-report ${resizing ? 'is-resizing' : ''}`}
      hidden={hidden}
      style={{ height }}
    >
      <div
        className="af-report-resizer"
        role="separator"
        aria-label="Wysokość przebiegu testu"
        aria-orientation="horizontal"
        aria-valuemin={160}
        aria-valuemax={maximum}
        aria-valuenow={height}
        tabIndex={0}
        title="Przeciągnij, aby zmienić wysokość przebiegu. Możesz też użyć strzałek góra/dół."
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          event.preventDefault();
          event.currentTarget.focus();
          event.currentTarget.setPointerCapture(event.pointerId);
          drag.current = {
            y: event.clientY,
            height: panel.current!.getBoundingClientRect().height,
          };
          setResizing(true);
        }}
        onPointerMove={(event) => {
          if (drag.current) setHeight(clamp(drag.current.height + drag.current.y - event.clientY));
        }}
        onLostPointerCapture={() => {
          drag.current = null;
          setResizing(false);
        }}
        onPointerUp={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId))
            event.currentTarget.releasePointerCapture(event.pointerId);
        }}
        onKeyDown={(event) => {
          if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return;
          event.preventDefault();
          setHeight((current) =>
            clamp(
              event.key === 'Home'
                ? 160
                : event.key === 'End'
                  ? window.innerHeight
                  : current + (event.key === 'ArrowUp' ? 32 : -32),
            ),
          );
        }}
      >
        <span />
      </div>
      {children}
    </div>
  );
}
