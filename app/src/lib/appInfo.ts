export const APP_NAME = "Potato";

declare const __APP_VERSION__: string | undefined;

/** 构建期由 vite define 注入 package.json 的 version;测试环境回落 dev。 */
export const APP_VERSION =
  typeof __APP_VERSION__ === "string" ? __APP_VERSION__ : "dev";
