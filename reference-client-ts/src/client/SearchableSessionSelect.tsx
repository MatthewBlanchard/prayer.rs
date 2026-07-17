import { useEffect, useId, useRef, useState } from "react";
import type { SessionState } from "./SessionsPanel.js";

interface SearchableSessionSelectProps {
  sessions: SessionState[];
  value: string | null;
  onChange: (value: string) => void;
  disabled?: boolean;
  ariaLabel?: string;
}

export default function SearchableSessionSelect({
  sessions,
  value,
  onChange,
  disabled = false,
  ariaLabel = "Session",
}: SearchableSessionSelectProps) {
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const filtered = sessions.filter((session) => session.sessionHandle.toLowerCase().includes(query.trim().toLowerCase()));

  useEffect(() => {
    function closeOnOutsideClick(event: MouseEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
        setQuery("");
      }
    }
    document.addEventListener("mousedown", closeOnOutsideClick);
    return () => document.removeEventListener("mousedown", closeOnOutsideClick);
  }, []);

  function choose(nextValue: string) {
    onChange(nextValue);
    setOpen(false);
    setQuery("");
  }

  return (
    <div className="searchable-session-select" ref={rootRef}>
      <input
        aria-label={ariaLabel}
        aria-autocomplete="list"
        aria-controls={listId}
        aria-expanded={open}
        role="combobox"
        disabled={disabled}
        value={open ? query : (value ?? "")}
        placeholder="Type to search sessions"
        onFocus={() => {
          setOpen(true);
          setQuery("");
          setActiveIndex(Math.max(0, sessions.findIndex((session) => session.sessionHandle === value)));
        }}
        onChange={(event) => {
          setQuery(event.target.value);
          setOpen(true);
          setActiveIndex(0);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            setOpen(false);
            setQuery("");
          } else if (event.key === "ArrowDown") {
            event.preventDefault();
            setOpen(true);
            setActiveIndex((index) => Math.min(index + 1, Math.max(0, filtered.length - 1)));
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setActiveIndex((index) => Math.max(0, index - 1));
          } else if (event.key === "Enter" && open && filtered[activeIndex]) {
            event.preventDefault();
            choose(filtered[activeIndex].sessionHandle);
          }
        }}
      />
      <span className="searchable-session-select__chevron" aria-hidden="true" />
      {open && (
        <div className="searchable-session-select__menu" id={listId} role="listbox">
          {filtered.length ? (
            filtered.map((session, index) => (
              <button
                type="button"
                role="option"
                aria-selected={session.sessionHandle === value}
                data-active={index === activeIndex}
                key={session.sessionHandle}
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => setActiveIndex(index)}
                onClick={() => choose(session.sessionHandle)}
              >
                <span>{session.sessionHandle}</span>
                {session.sessionHandle === value && <span aria-hidden="true">✓</span>}
              </button>
            ))
          ) : (
            <div className="searchable-session-select__empty">No matching sessions</div>
          )}
        </div>
      )}
    </div>
  );
}
