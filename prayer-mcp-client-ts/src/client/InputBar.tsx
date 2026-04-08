import { useRef, KeyboardEvent } from "react";

interface InputBarProps {
  onSubmit: (content: string) => void;
  disabled: boolean;
  error: string | null;
}

export default function InputBar({ onSubmit, disabled, error }: InputBarProps) {
  const ref = useRef<HTMLTextAreaElement>(null);

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      const value = ref.current?.value.trim() ?? "";
      if (!value || disabled) return;
      if (ref.current) ref.current.value = "";
      onSubmit(value);
    }
  }

  return (
    <div className="input-bar">
      {error && <div className="input-error">{error}</div>}
      <div className="input-row">
        <span className="input-prefix">you&gt;</span>
        <textarea
          ref={ref}
          className="input-textarea"
          placeholder={disabled ? "Waiting..." : "Type a message… (Enter to send, Shift+Enter for newline, /clear to reset)"}
          disabled={disabled}
          onKeyDown={handleKeyDown}
          rows={2}
          autoFocus
        />
      </div>
    </div>
  );
}
