import { invoke } from "@tauri-apps/api/core";

import type { LogEntry, LogLevel } from "./generated/bindings";

export type LogContext = Record<string, string>;

export const logger = {
  debug(message: string, context?: LogContext) {
    return write("debug", message, context);
  },
  info(message: string, context?: LogContext) {
    return write("info", message, context);
  },
  warn(message: string, context?: LogContext) {
    return write("warn", message, context);
  },
  error(message: string, context?: LogContext) {
    return write("error", message, context);
  },
  entries() {
    return invoke<LogEntry[]>("get_logs");
  },
  clear() {
    return invoke<void>("clear_logs");
  },
};

function write(level: LogLevel, message: string, context?: LogContext) {
  return invoke<void>("write_log", { level, message, context });
}
