import type { MarketMovement } from "../src/generated/types.js";

declare const movement: MarketMovement;
movement.createdAtUnix;
// Rust's camelCase wire rename is authoritative; snake_case must not compile.
// @ts-expect-error generated public contracts reject Rust field spelling
movement.created_at_unix;
