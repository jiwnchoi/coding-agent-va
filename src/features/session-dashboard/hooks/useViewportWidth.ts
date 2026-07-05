import { useSyncExternalStore } from "react";

function getViewportWidth() {
  return typeof window === "undefined" ? 0 : window.innerWidth;
}

export function useViewportWidth() {
  return useSyncExternalStore(
    (onStoreChange) => {
      window.addEventListener("resize", onStoreChange);

      return () => {
        window.removeEventListener("resize", onStoreChange);
      };
    },
    getViewportWidth,
    () => 0
  );
}
