import { useEffect, useState } from "react";
import type { Squad } from "../../shared/types.js";
import { fetchSquads } from "../api.js";

export function useSquads(): Squad[] {
  const [squads, setSquads] = useState<Squad[]>([]);

  useEffect(() => {
    let active = true;
    const refresh = () => {
      void fetchSquads()
        .then((next) => {
          if (active) setSquads(next);
        })
        .catch(() => {
          // Map decoration is best-effort; squad management surfaces report API errors.
        });
    };
    refresh();
    window.addEventListener("prayer-squads-updated", refresh);
    return () => {
      active = false;
      window.removeEventListener("prayer-squads-updated", refresh);
    };
  }, []);

  return squads;
}
