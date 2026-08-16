export const APP_NAME = "Potato";

declare const __APP_VERSION__: string | undefined;

/** 构建期由 vite 注入 potato.__version__;测试环境回落 2.0.5。 */
export const APP_VERSION =
  typeof __APP_VERSION__ === "string" && __APP_VERSION__ !== "0.1.0"
    ? __APP_VERSION__
    : "2.0.5";
