import { FormEvent, useEffect, useMemo, useState } from "react";
import { actions } from "@prayer/sdk";
import { GameChatChannel, GameChatMessage } from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectGameChatMessages } from "./prayer/worldSelectors.js";

type GameChatPanelProps = { sessions: SessionState[] };
const FILTER_CHANNELS: GameChatChannel[] = ["system", "local", "faction", "emergency"];
const SEND_CHANNELS: Array<Exclude<GameChatChannel, "emergency">> = ["system", "local", "faction", "private"];

function messageKey(message: GameChatMessage): string {
  return message.id || `${message.channel}:${message.timestampUtc}:${message.senderId}:${message.content}`;
}

function formatTimestamp(iso: string): string {
  const date = Date.parse(iso);
  if (!Number.isFinite(date)) return iso || "?";
  return new Date(date).toLocaleString([], { month: "short", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function locationLabel(message: GameChatMessage): string {
  if (message.systemId && message.poiId) return `${message.systemId} / ${message.poiId}`;
  return message.systemId || message.poiId || "unknown location";
}

function channelLabel(channel: string): string {
  return channel === "private" ? "dm" : channel;
}

export default function GameChatPanel({ sessions }: GameChatPanelProps) {
  const prayer = usePrayer();
  const [errors, setErrors] = useState<string[]>([]);
  const [channelFilter, setChannelFilter] = useState<GameChatChannel | "all">("all");
  const [search, setSearch] = useState("");
  const [sendHandle, setSendHandle] = useState("");
  const [sendChannel, setSendChannel] = useState<Exclude<GameChatChannel, "emergency">>("system");
  const [targetId, setTargetId] = useState("");
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const sessionHandles = useMemo(() => sessions.map((session) => session.sessionHandle).sort(), [sessions]);
  const selectedSession = sessions.find((session) => session.sessionHandle === sendHandle) ?? sessions[0];
  const messages = useMemo(
    () => (selectedSession ? selectGameChatMessages(prayer.chatMessagesBySession[selectedSession.botId] ?? [], selectedSession.sessionHandle) : []),
    [prayer.chatMessagesBySession, selectedSession],
  );
  const loading = prayer.connection === "connecting";

  useEffect(() => {
    if (!sendHandle || !sessionHandles.includes(sendHandle)) setSendHandle(sessionHandles[0] ?? "");
  }, [sendHandle, sessionHandles]);

  const visibleMessages = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return messages.filter(
      (message) =>
        (channelFilter === "all" || message.channel === channelFilter) &&
        (!needle ||
          [message.sender, message.senderId, message.content, message.channel, message.systemId, message.poiId, message.factionId, message.targetName].some(
            (field) => field?.toLowerCase().includes(needle),
          )),
    );
  }, [channelFilter, messages, search]);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const content = draft.trim();
    if (!sendHandle || !content || sending) return;
    setSending(true);
    try {
      const bot = await prayer.bot(sendHandle);
      const run = await bot.start([actions.say({ channel: sendChannel, content, target: sendChannel === "private" ? targetId.trim() : null })], {
        idempotencyKey: crypto.randomUUID(),
      });
      const terminal = await run.wait();
      if (terminal.status !== "succeeded") throw new Error(`Chat run ${run.id} ${terminal.status}.`);
      setDraft("");
      await prayer.refresh();
    } catch (error) {
      setErrors((current) => [`send: ${error instanceof Error ? error.message : String(error)}`, ...current].slice(0, 6));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="game-chat-panel">
      <div className="game-chat-toolbar">
        <div className="game-chat-heading">
          <div className="game-chat-title">Universe Comms</div>
          <div className="game-chat-meta">
            {messages.length} messages · {sessionHandles.length} bots · {loading ? "refreshing" : "live"}
          </div>
        </div>
        <div className="game-chat-filters">
          <select value={sendHandle} onChange={(event) => setSendHandle(event.target.value)} disabled={!sessionHandles.length} aria-label="Read chat as bot">
            {sessionHandles.length ? sessionHandles.map((handle) => <option key={handle}>{handle}</option>) : <option value="">no bots</option>}
          </select>
          <select value={channelFilter} onChange={(event) => setChannelFilter(event.target.value as GameChatChannel | "all")} aria-label="Filter channel">
            <option value="all">all channels</option>
            {FILTER_CHANNELS.map((channel) => (
              <option key={channel} value={channel}>
                {channelLabel(channel)}
              </option>
            ))}
          </select>
          <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Filter log" aria-label="Filter chat log" />
          <button className="session-btn" onClick={() => void prayer.refresh()} disabled={loading}>
            refresh
          </button>
        </div>
      </div>
      <div className="game-chat-log" role="log" aria-live="polite">
        {visibleMessages.length ? (
          visibleMessages.map((message) => (
            <article className="game-chat-message" key={messageKey(message)}>
              <div className="game-chat-message-meta">
                <span className="game-chat-message-time">{formatTimestamp(message.timestampUtc)}</span>
                <span className="game-chat-message-channel">{channelLabel(message.channel)}</span>
                <span className="game-chat-message-location">{locationLabel(message)}</span>
                {message.empireOfficial && <span className="game-chat-message-official">official</span>}
              </div>
              <div className="game-chat-message-body">
                <span className="game-chat-message-sender">{message.sender}</span>
                <span className="game-chat-message-content">{message.content}</span>
              </div>
            </article>
          ))
        ) : (
          <div className="game-chat-empty">{sessionHandles.length ? "No chat messages found." : "No sessions registered."}</div>
        )}
      </div>
      {errors.length > 0 && (
        <div className="game-chat-errors">
          {errors.slice(0, 4).map((error) => (
            <span key={error}>{error}</span>
          ))}
        </div>
      )}
      <form className="game-chat-compose" onSubmit={handleSubmit}>
        <select value={sendHandle} onChange={(event) => setSendHandle(event.target.value)} disabled={!sessionHandles.length} aria-label="Send as bot">
          {sessionHandles.length ? sessionHandles.map((handle) => <option key={handle}>{handle}</option>) : <option value="">no bots</option>}
        </select>
        <select value={sendChannel} onChange={(event) => setSendChannel(event.target.value as Exclude<GameChatChannel, "emergency">)} aria-label="Send channel">
          {SEND_CHANNELS.map((channel) => (
            <option key={channel} value={channel}>
              {channelLabel(channel)}
            </option>
          ))}
        </select>
        {sendChannel === "private" && (
          <input value={targetId} onChange={(event) => setTargetId(event.target.value)} placeholder="Target player" aria-label="Private message target" />
        )}
        <input
          className="game-chat-compose-input"
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="Message"
          maxLength={500}
          aria-label="Chat message"
        />
        <button className="session-btn" type="submit" disabled={sending || !sendHandle || !draft.trim() || (sendChannel === "private" && !targetId.trim())}>
          send
        </button>
      </form>
    </div>
  );
}
