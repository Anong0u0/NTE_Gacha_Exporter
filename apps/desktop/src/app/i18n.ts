import type { Ref } from "vue";

import enMessages from "../assets/i18n/en.json";
import zhCnMessages from "../assets/i18n/zh-CN.json";
import zhHantMessages from "../assets/i18n/zh-Hant.json";

const en = enMessages;
type EnMessages = typeof en;
export type I18nKey = keyof EnMessages;
type Messages = Record<I18nKey, string>;
type I18nParams = Record<string, string | number | boolean | null | undefined>;

const dictionaries: Record<string, Messages> = {
  en,
  "zh-CN": zhCnMessages,
  "zh-Hant": zhHantMessages,
};

export function createTranslator(uiLocale: Ref<string>) {
  return (key: I18nKey, params?: I18nParams) => translate(uiLocale.value, key, params);
}

function translate(locale: string, key: I18nKey, params?: I18nParams) {
  const messages = dictionaries[locale];
  return interpolate(messages?.[key] ?? "", params);
}

export function uiLocaleDisplayName(locale: string) {
  if (locale === "zh-CN") return "简体中文";
  if (locale === "zh-Hant") return "繁體中文";
  if (locale === "en") return "English";
  return "";
}

function interpolate(template: string, params?: I18nParams) {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(params[key] ?? ""));
}
