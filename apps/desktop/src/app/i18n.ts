import type { Ref } from "vue";

import enMessages from "../assets/i18n/en.json";
import zhHantMessages from "../assets/i18n/zh-Hant.json";

export const fallbackUiLocale = "en";

const en = enMessages;
type EnMessages = typeof en;
export type I18nKey = keyof EnMessages;
type Messages = Record<I18nKey, string>;
type I18nParams = Record<string, string | number | boolean | null | undefined>;

const dictionaries: Record<string, Messages> = {
  en,
  "zh-Hant": zhHantMessages,
};

export const dictionaryUiLocales = Object.keys(dictionaries);

export function createTranslator(uiLocale: Ref<string>) {
  return (key: I18nKey, params?: I18nParams) => translate(uiLocale.value, key, params);
}

export function translate(locale: string, key: I18nKey, params?: I18nParams) {
  const messages = dictionaries[locale] ?? dictionaries[fallbackUiLocale];
  return interpolate(messages[key] ?? en[key] ?? key, params);
}

export function uiLocaleHasDictionary(locale: string) {
  return locale in dictionaries;
}

export function uiLocaleDisplayName(locale: string, t: (key: I18nKey, params?: I18nParams) => string) {
  if (locale === "zh-Hant") return t("locale.zhHant");
  if (locale === "en") return t("locale.en");
  return `${locale} (${t("locale.en")})`;
}

function interpolate(template: string, params?: I18nParams) {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(params[key] ?? ""));
}
