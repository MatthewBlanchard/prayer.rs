import { Express } from "express";

/**
 * Application-local per-bot resources plus the final read projection awaiting
 * migration into a browser selector. Ordinary Prayer state and control routes
 * intentionally do not live on the Node host.
 */
export function registerSessionRoutes(_app: Express): void {}
