import { createContext, useContext } from "react";

import { ja } from "./ja";
import { en } from "./en";

/**
 * The top row stays English in every language.
 *
 * The buttons on a CDJ are English, and a tool used in front of one reads
 * better matching them. Only words a non-English speaker already knows.
 */
export const buttons = {
  tracks: "CONVERT",
  drive: "USB CHECK",
  devices: "DEVICES",
} as const;

/** The shape every language has to fill. Japanese is the reference. */
export type Strings = typeof ja;

export const LANGUAGES = { ja, en };

export type Language = keyof typeof LANGUAGES;

/** What the user picked. `auto` follows whatever the machine is set to. */
export type Choice = Language | "auto";

export const CHOICES: Choice[] = ["auto", "ja", "en"];

/** How each choice names itself, so the menu is readable before you switch. */
export const CHOICE_NAMES: Record<Language, string> = {
  ja: "日本語",
  en: "English",
};

/**
 * Resolve a choice against the machine's locale.
 *
 * Anything that is not Japanese gets English: it is the language the rest of
 * the manuals and the error codes are in, so it is the safer fallback.
 */
export function resolve(choice: Choice, locale: string | null): Strings {
  if (choice !== "auto") return LANGUAGES[choice];
  return locale?.toLowerCase().startsWith("ja") === true ? ja : en;
}

const Current = createContext<Strings>(ja);

export const StringsProvider = Current.Provider;

export const useStrings = () => useContext(Current);
