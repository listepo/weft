import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { FILE_NAME } from "./formats.ts";
import type { Assertion, Format, Ref } from "./neutral.ts";

export const CORPUS_DIR = fileURLToPath(new URL("../../corpus/", import.meta.url));

export const SCREENS = [
  "login",
  "signup",
  "settings",
  "data-table",
  "tabs",
  "confirm-dialog",
  "wizard-step",
  "search-results",
  "todo-list",
  "profile",
  "menu",
  "error-state",
] as const;

export const readScreen = (screen: string, format: Format): string =>
  readFileSync(`${CORPUS_DIR}${screen}/${FILE_NAME[format]}`, "utf8");

export interface EditTask {
  id: string;
  screen: string;
  type: "edit";
  instruction: string;
  expect: Assertion[];
  /** Nodes the edit legitimately removes or renames; everything else must survive. */
  remove?: Ref[];
}

export interface QuestionTask {
  id: string;
  screen: string;
  type: "question";
  question: string;
  /** Accepted final answers, compared case-insensitively. */
  answer: string[];
}

export type Task = EditTask | QuestionTask;

export const loadTasks = (): Task[] =>
  JSON.parse(readFileSync(`${CORPUS_DIR}tasks.json`, "utf8")) as Task[];
