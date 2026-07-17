export function formatCredits(value: number | null | undefined, fallback = "-"): string {
  if (value == null || !Number.isFinite(value)) return fallback;
  return `${Math.round(value).toLocaleString()} cr`;
}

export function CreditAmount({ value, fallback }: { value: number | null | undefined; fallback?: string }) {
  if (value == null || !Number.isFinite(value)) {
    return <span className="credit-amount">{fallback ?? "-"}</span>;
  }
  return (
    <span className="credit-amount">
      {Math.round(value).toLocaleString()}
      <span className="credit-suffix">cr</span>
    </span>
  );
}

export function CreditPair({ buy, sell }: { buy: number | null; sell: number | null }) {
  return (
    <>
      <CreditAmount value={buy} />/<CreditAmount value={sell} />
    </>
  );
}
