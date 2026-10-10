import { useEffect, useState } from "react";

// The open courses, one browser-history entry each, so the Android back button (or a mouse's back button) goes up one
// level instead of leaving the app. history.state.depth is how many courses were open when that entry was made.
export function useCourseStack<T>() {
  const [stack, setStack] = useState<T[]>([]);
  useEffect(() => {
    if (stack.length > (history.state?.depth ?? 0)) history.pushState({ depth: stack.length }, "");
  }, [stack.length]);
  useEffect(() => {
    const f = (e: PopStateEvent) => setStack((s) => s.slice(0, e.state?.depth ?? 0));
    window.addEventListener("popstate", f);
    return () => window.removeEventListener("popstate", f);
  }, []);
  const back = () => history.back();
  // leaving for another screen closes every open course at once, and takes their history entries with them
  const clear = () => (history.state?.depth ? history.go(-history.state.depth) : setStack([]));
  return [stack, setStack, back, clear] as const;
}
