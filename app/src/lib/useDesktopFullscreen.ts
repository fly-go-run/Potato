import { useEffect, useState } from "react";
import {
  getDesktopFullscreen,
  isDesktopShell,
  listenDesktopEvent,
} from "./desktop";

/**
 * 桌面壳原生全屏态。mac overlay 红绿灯在全屏时被系统藏起,
 * 顶栏不再给灯簇留空。
 */
export function useDesktopFullscreen(): boolean {
  const [fullscreen, setFullscreen] = useState(false);

  useEffect(() => {
    if (!isDesktopShell()) return;

    let cancelled = false;

    const sync = () => {
      void getDesktopFullscreen().then((value) => {
        if (!cancelled) setFullscreen(value);
      });
    };

    sync();

    const onResize = () => sync();
    window.addEventListener("resize", onResize);

    let unlisten: (() => void) | null = null;
    void listenDesktopEvent("tauri://resize", onResize).then((fn) => {
      if (cancelled) {
        fn?.();
        return;
      }
      unlisten = fn;
    });

    return () => {
      cancelled = true;
      window.removeEventListener("resize", onResize);
      unlisten?.();
    };
  }, []);

  return fullscreen;
}
