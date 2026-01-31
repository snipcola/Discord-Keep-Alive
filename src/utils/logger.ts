import { styleText } from "util";

export function logSuccess(text: string) {
  console.log(styleText("green", text));
}

export function logError(text: string) {
  console.log(styleText("red", text));
}
