import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import { TranscriptItem } from "../shared/types.js";

const PRAYER_FLOW_KEYWORDS = new Set(["if", "until"]);
const PRAYER_ACTION_KEYWORDS = new Set([
  "halt", "mine", "dock", "stash", "buy", "sell", "go", "wait",
  "accept_mission", "abandon_mission", "craft", "jettison",
  "repair", "refuel", "switch_ship", "install_mod", "buy_ship",
  "survey", "explore",
]);

function PrayerToken({ token }: { token: string }) {
  const norm = token.replace(/^[^a-z0-9_$]+|[^a-z0-9_$]+$/gi, "").toLowerCase();

  if (token.startsWith("$")) {
    return <span className="prayer-var">{token}</span>;
  }
  if (/^\d+$/.test(norm)) {
    return <span className="prayer-num">{token}</span>;
  }
  if (PRAYER_FLOW_KEYWORDS.has(norm)) {
    return <span className="prayer-flow">{token}</span>;
  }
  if (PRAYER_ACTION_KEYWORDS.has(norm)) {
    return <span className="prayer-action">{token}</span>;
  }
  if (/^[A-Z_]+\(\)$/.test(token)) {
    return <span className="prayer-macro">{token}</span>;
  }
  if ("{}();,".includes(token)) {
    return <span className="prayer-punct">{token}</span>;
  }
  return <span>{token}</span>;
}

function PrayerLine({ line }: { line: string }) {
  // Split on whitespace and punctuation, keep delimiters
  const tokens = line.split(/([{}();,\s]+)/);
  return (
    <>
      {tokens.map((tok, i) =>
        tok.trim() === "" ? (
          <span key={i}>{tok}</span>
        ) : (
          <PrayerToken key={i} token={tok} />
        )
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// <thinking> collapsible blocks
// ---------------------------------------------------------------------------

type ContentSegment =
  | { kind: "text"; content: string }
  | { kind: "thinking"; content: string };

function splitThinkingBlocks(content: string): ContentSegment[] {
  const segments: ContentSegment[] = [];
  const re = /<thinking>([\s\S]*?)<\/thinking>/g;
  let last = 0;
  let match: RegExpExecArray | null;

  while ((match = re.exec(content)) !== null) {
    if (match.index > last) {
      segments.push({ kind: "text", content: content.slice(last, match.index) });
    }
    segments.push({ kind: "thinking", content: match[1] });
    last = match.index + match[0].length;
  }

  if (last < content.length) {
    segments.push({ kind: "text", content: content.slice(last) });
  }

  return segments;
}

function ThinkingBlock({ content }: { content: string }) {
  return (
    <details className="thinking-block">
      <summary className="thinking-summary">Thinking</summary>
      <div className="thinking-content">
        <TextContent content={content} />
      </div>
    </details>
  );
}

function CodeBlock({ lang, codeText }: { lang: string; codeText: string }) {
  const isPrayer = lang === "prayer" || lang === "prayerlang";
  const lines = codeText.split("\n");
  // Strip trailing empty line that markdown parsers often add
  if (lines[lines.length - 1] === "") lines.pop();
  return (
    <div className="code-block">
      <div className="code-block-header">{lang || "code"}</div>
      <pre className={isPrayer ? "code-prayer" : "code-generic"}>
        {lines.map((cl, i) => (
          <div key={i} className="code-line">
            {isPrayer ? <PrayerLine line={cl} /> : cl}
          </div>
        ))}
      </pre>
    </div>
  );
}

const mdComponents: React.ComponentProps<typeof ReactMarkdown>["components"] = {
  // Fenced code blocks
  pre({ children }) {
    // ReactMarkdown wraps code in <pre><code className="language-xxx">
    // We intercept at the <pre> level to grab both lang and text
    const codeEl = (children as React.ReactElement[] | undefined)?.[0];
    const className: string = (codeEl?.props as Record<string, unknown>)?.["className"] as string ?? "";
    const lang = className.replace("language-", "").toLowerCase();
    const codeText = String((codeEl?.props as Record<string, unknown>)?.["children"] ?? "");
    return <CodeBlock lang={lang} codeText={codeText} />;
  },
  // Inline code
  code({ children }) {
    return <code className="inline-code">{children}</code>;
  },
  // Wrap paragraphs in assistant-line style
  p({ children }) {
    return <div className="assistant-line">{children}</div>;
  },
};

function TextContent({ content }: { content: string }) {
  return <ReactMarkdown components={mdComponents}>{content}</ReactMarkdown>;
}

function AssistantContent({ content }: { content: string }) {
  const segments = splitThinkingBlocks(content);
  return (
    <>
      {segments.map((seg, i) =>
        seg.kind === "thinking" ? (
          <ThinkingBlock key={i} content={seg.content} />
        ) : (
          <TextContent key={i} content={seg.content} />
        )
      )}
    </>
  );
}


function ToolDropdown({ label, content }: { label: string; content: string }) {
  const trimmed = content.trim();
  if (!trimmed) return null;
  return (
    <details className="tool-card-details">
      <summary className="tool-card-summary">{label}</summary>
      <pre className="tool-card-body">{trimmed}</pre>
    </details>
  );
}

function ToolCard({
  item,
}: {
  item: Extract<TranscriptItem, { kind: "tool_card" }>;
}) {
  const label = item.status === "error" ? "err" : item.status;

  return (
    <div className={`tool-card tool-card--${item.status}`}>
      <div className="tool-card-header">
        <span className="tool-card-label">[tool/{label}]</span>
        <span className="tool-card-name">{item.name}</span>
      </div>
      <ToolDropdown label="input" content={item.argsPreview} />
      {item.resultPreview !== null && (
        <ToolDropdown label="output" content={item.resultPreview} />
      )}
    </div>
  );
}

interface ChatPaneProps {
  items: TranscriptItem[];
  busy: boolean;
}

export default function ChatPane({ items, busy }: ChatPaneProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [items.length, busy]);

  return (
    <div className="chat-pane">
      {items.map((item, idx) => {
        switch (item.kind) {
          case "user":
            return (
              <div key={idx} className="message message--user">
                <span className="message-prefix user-prefix">you&gt;</span>
                <span className="message-content">{item.content}</span>
              </div>
            );

          case "assistant":
            return (
              <div key={idx} className="message message--assistant">
                <span className="message-prefix assistant-prefix">assistant&gt;</span>
                <div className="message-content">
                  <AssistantContent content={item.content} />
                </div>
              </div>
            );

          case "tool_card":
            return <ToolCard key={idx} item={item} />;

          case "error":
            return (
              <div key={idx} className="message message--error">
                <span className="message-prefix">error&gt;</span>
                <span className="message-content">{item.message}</span>
              </div>
            );
        }
      })}

      {busy && (
        <div className="thinking-indicator">
          <span className="thinking-dot" />
          <span className="thinking-dot" />
          <span className="thinking-dot" />
        </div>
      )}

      <div ref={bottomRef} />
    </div>
  );
}
