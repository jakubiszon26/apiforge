import { useEffect, useRef } from 'react';

/** Keep keyboard navigation in the active dialog and return focus to its opener. */
export function useModalFocus<T extends HTMLElement>(onClose: () => void) {
  const element = useRef<T>(null);
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);
  useEffect(() => {
    const dialog = element.current;
    if (!dialog) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusable = () =>
      Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not([disabled]),input:not([disabled]),textarea:not([disabled]),select:not([disabled]),a[href],[tabindex="0"]',
        ),
      ).filter((node) => node.getClientRects().length > 0);
    (dialog.querySelector<HTMLElement>('[data-autofocus]') ?? focusable()[0] ?? dialog).focus();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        close.current();
        return;
      }
      if (event.key !== 'Tab') return;
      const elements = focusable(),
        first = elements[0],
        last = elements.at(-1);
      if (!first) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      if (
        event.shiftKey &&
        (document.activeElement === first || !dialog.contains(document.activeElement))
      ) {
        event.preventDefault();
        last?.focus();
      } else if (
        !event.shiftKey &&
        (document.activeElement === last || !dialog.contains(document.activeElement))
      ) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', keydown);
    return () => {
      document.removeEventListener('keydown', keydown);
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  return element;
}
