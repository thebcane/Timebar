import { useEffect } from "react";
import { useStore } from "./store";

/** Run the store's listeners/ticker for the lifetime of a window root. */
export function useAppInit() {
  const init = useStore((s) => s.init);
  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;
    init().then((c) => {
      if (mounted) cleanup = c;
      else c();
    });
    return () => {
      mounted = false;
      cleanup?.();
    };
  }, [init]);
}
